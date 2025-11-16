use anyhow::{Context, Result};
use blink_timer::{
    APP_NAME, ActivityMessage, DurationExt, IpcRequest, IpcResponse, Status,
    async_socket::{SocketServer, SocketStream},
    config::{Config, Timer},
    get_unix_time, util,
};
use clap::Parser;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::signal::unix::{SignalKind, signal};

const TICK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Set a custom config file
    #[clap(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();
    let args = Args::parse();

    let config_path = args.config.unwrap_or({
        dirs::config_dir()
            .context("no config dir available")?
            .join(APP_NAME)
            .join(APP_NAME.to_string() + ".yaml")
    });
    log::debug!("Config path: '{}'", config_path.display());
    let config = Config::load_or_create(config_path)?;

    Daemon::new(config).run().await?;
    Ok(())
}

#[derive(Default, Clone)]
struct TimerState {
    time_left: Duration,
    prompts: u64,
    timer: Timer,
}

impl TimerState {
    fn new(b: Timer) -> Self {
        Self {
            timer: b,
            ..Default::default()
        }
    }

    fn reset(&mut self) {
        self.prompts = 0;
    }
}

struct Daemon {
    config: Config,
    elapsed: Duration,
    last_update: Instant,
    next_timer_at: Duration,
    next_timer: Option<Timer>,
    timers: Vec<TimerState>,
    is_enabled: bool,
    is_frozen: bool,
    last_input: u64,
}

impl Daemon {
    fn new(config: Config) -> Self {
        let state = config
            .timers
            .iter()
            .map(|b| TimerState::new(b.clone()))
            .collect();
        Self {
            config,
            elapsed: Duration::ZERO,
            last_update: Instant::now(),
            next_timer_at: Duration::MAX,
            next_timer: None,
            timers: state,
            is_enabled: true,
            is_frozen: false,
            last_input: get_unix_time(),
        }
    }

    async fn run(self) -> Result<()> {
        let mut listener = SocketServer::create(blink_timer::socket_path(), true)
            .await
            .context("failed to create socket server")?;
        let mut activity_stream = if self.config.input_tracking.is_some() {
            Some(SocketStream::connect(blink_timer::actived_socket_path()).await?)
        } else {
            None
        };

        util::show_notification(
            "Blink".to_string(),
            "Blink is running.".to_string(),
            Duration::from_secs(5),
            0,
        );

        let mut sigterm = signal(SignalKind::terminate())?;
        let mut sigint = signal(SignalKind::interrupt())?;
        let daemon = Arc::new(Mutex::new(self));
        daemon.lock().unwrap().update_timer();

        tokio::spawn({
            let daemon = daemon.clone();
            async move {
                let mut interval = tokio::time::interval(TICK_INTERVAL);
                interval.tick().await; // First tick completes immediately
                loop {
                    interval.tick().await;
                    let now = Instant::now();
                    let mut daemon = daemon.lock().unwrap();
                    daemon.tick(now);
                    drop(daemon);
                }
            }
        });

        loop {
            tokio::select! {
                _ = sigterm.recv() => {
                    log::info!("Received SIGTERM, shutting down");
                    break;
                }
                _ = sigint.recv() => {
                    log::info!("Received SIGINT, shutting down");
                    break;
                }
                Ok(client_stream) = listener.accept_client() => {
                    tokio::spawn({
                        let daemon = daemon.clone();
                        async move {
                            if let Err(e) = Self::handle_client(client_stream, daemon).await {
                                log::error!("Failed to handle client: {e:?}");
                            }
                        }
                    });
                }
                activity_msg = async {
                    if let Some(stream) = &mut activity_stream {
                        stream.recv::<ActivityMessage>().await
                    } else {
                        std::future::pending().await
                    }
                } => match activity_msg {
                    Ok(activity) => {
                        let mut daemon = daemon.lock().unwrap();
                        daemon.last_input = activity.last_input;
                    }
                    Err(e) => log::error!("Failed to receive activity message: {e:?}")
                }
            }
        }
        Ok(())
    }

    fn tick(&mut self, now: Instant) {
        let delta = now.duration_since(self.last_update);
        self.last_update = now;

        // Check for big delay between ticks, likely caused when the system was suspended
        // This also counts as input inactivity
        if self
            .config
            .input_tracking
            .as_ref()
            .map(|i| i.reset_after)
            .is_some_and(|timeout_reset| delta >= timeout_reset)
        {
            if self.elapsed > Duration::ZERO {
                log::info!("Resetting timer (update delta of {})", delta.display());
                self.reset();
                return;
            }
        } else if let Some(input_tracking) = &self.config.input_tracking {
            // Reset or freeze the timer based on input tracking config
            let elapsed_since_input =
                Duration::from_secs(blink_timer::get_unix_time() - self.last_input);
            if elapsed_since_input >= input_tracking.reset_after {
                if self.elapsed > Duration::ZERO {
                    log::info!("Resetting timer (input timeout {elapsed_since_input:?})");
                    self.reset();
                    self.is_frozen = true;
                    return;
                }
            }
            if !self.is_frozen && elapsed_since_input > input_tracking.pause_after {
                log::trace!("Frozen");
                self.is_frozen = true;
            } else if self.is_frozen && elapsed_since_input < Duration::from_secs(3) {
                log::trace!("Unfrozen");
                self.is_frozen = false;
            }
        };

        if !self.is_frozen && self.is_enabled {
            self.elapsed += delta;
            log::trace!(
                "Tick {}/{}",
                self.elapsed.display(),
                self.next_timer_at.display()
            );
        }

        if self.elapsed >= self.next_timer_at {
            self.notify();
            self.update_timer();
        }
    }

    fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
        for item in self.timers.iter_mut() {
            item.reset();
        }
        self.update_timer();
    }

    fn update_timer(&mut self) {
        // Determine the next timer
        for item in self.timers.iter_mut() {
            item.time_left = if let Some(initial_delay) = item.timer.initial_delay
                && self.elapsed <= initial_delay
            {
                // Before the initial delay
                initial_delay - self.elapsed
            } else if let Some(initial_delay) = item.timer.initial_delay {
                // After the initial delay: at every interval, relative to when initial delay ended
                let elapsed_since_initial = self.elapsed - initial_delay;
                item.timer.interval
                    - Duration::from_secs_f64(
                        elapsed_since_initial.as_secs_f64() % item.timer.interval.as_secs_f64(),
                    )
            } else {
                // No initial delay: at every interval from the start
                item.timer.interval
                    - Duration::from_secs_f64(
                        self.elapsed.as_secs_f64() % item.timer.interval.as_secs_f64(),
                    )
            }
        }

        // Sort first by time left and then by interval (timers with longer intervals have priority)
        self.timers.sort_by(|a, b| {
            a.time_left
                .cmp(&b.time_left)
                .then(b.timer.interval.cmp(&a.timer.interval))
        });

        // The first timer is the next one
        if let Some(next) = self.timers.first_mut() {
            // The decline function, the interval will be multiplied by 0.5 with a decline of 1.0
            let interval_mult = (1.0 / (1.0 + next.timer.decline)).powf(next.prompts as f64);
            let interval = Duration::from_secs_f64(next.time_left.as_secs_f64() * interval_mult);
            log::debug!(
                "Next interval: {}, multiplier: {} (prompt: {})",
                interval.display(),
                interval_mult,
                next.prompts
            );
            next.prompts += 1;

            self.next_timer_at = self.elapsed + interval;
            self.next_timer = Some(next.timer.clone());
        } else {
            log::error!("No timers found! Make sure to specify at least one in the config.");
        }
    }

    fn notify(&self) {
        if let Some(timer) = self.next_timer.clone() {
            log::info!("Time to take a break!\x07");

            if let Some(notification) = timer.notification {
                let description = {
                    if !notification.descriptions.is_empty() {
                        let rand_index = rand::random_range(0..notification.descriptions.len());
                        &notification.descriptions[rand_index]
                    } else {
                        "{} elapsed"
                    }
                };
                let description =
                    util::format_string(description, &self.elapsed.display().to_string());
                util::show_notification(
                    notification.title,
                    description,
                    notification.timeout.unwrap_or(Duration::from_secs(10)),
                    1,
                );
            }

            if let Some(sound) = timer.sound {
                util::play_sound(sound);
            }

            if let Some(cmd) = timer.command {
                util::exec_command(cmd);
            }
        }
    }

    async fn handle_client(mut stream: SocketStream, daemon: Arc<Mutex<Daemon>>) -> Result<()> {
        let msg: IpcRequest = stream.recv().await?;
        let resp = {
            let mut daemon = daemon.lock().unwrap();
            daemon.handle_msg(msg)?
        };
        stream.send(resp).await?;
        Ok(())
    }

    fn handle_msg(&mut self, msg: IpcRequest) -> Result<IpcResponse> {
        Ok(match msg {
            IpcRequest::Status => {
                IpcResponse::Status(Status::new(self.elapsed, self.next_timer_at))
            }
            IpcRequest::Toggle => {
                self.is_enabled = !self.is_enabled;
                log::info!("Set enabled to: {}", self.is_enabled);
                IpcResponse::Ok
            }
            IpcRequest::Reset => {
                self.reset();
                IpcResponse::Ok
            }
        })
    }
}
