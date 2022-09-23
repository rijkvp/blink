use crate::{
    config::{Config, Timer},
    util,
};
use log::{debug, error, info, trace};
use rand::{thread_rng, Rng};
use std::{
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

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

pub struct App {
    elapsed: Duration,
    time_left: Duration,
    next_timer: Option<Timer>,
    timers: Vec<TimerState>,
    config: Config,
}

impl App {
    pub fn new(config: Config) -> Self {
        let state = config
            .timers
            .iter()
            .map(|b| TimerState::new(b.clone()))
            .collect();
        Self {
            elapsed: Duration::ZERO,
            time_left: Duration::MAX,
            next_timer: None,
            timers: state,
            config,
        }
    }

    pub fn run(&mut self) {
        let last_input = Arc::new(RwLock::new(Instant::now()));
        if self.config.input_tracking.is_some() {
            util::start_input_tracking(last_input.clone());
        }

        self.update_timer();
        let mut last_update = Instant::now();

        let mut paused = false;
        let mut reset = false;

        util::show_notification(
            "Blink".to_string(),
            "Blink is running.".to_string(),
            Duration::from_secs(10),
            0,
        );

        loop {
            let delta = last_update.elapsed();

            if delta >= self.config.timeout_reset {
                info!("Resetting timer (timeout)");
                reset = true;
            }

            if let Some(input_tracking) = &self.config.input_tracking {
                let input_elapsed = last_input.read().unwrap().elapsed();
                if input_elapsed >= input_tracking.inactivity_reset {
                    info!("Resetting timer (input inactivity)");
                    reset = true;
                }

                // Pause after input activity
                paused = input_elapsed > input_tracking.inactivity_pause;
            }
            if reset {
                self.reset();
                continue;
            }

            if !paused {
                self.elapsed += delta;
                trace!("elapsed: {:?}", self.elapsed);
            }

            if self.elapsed >= self.time_left {
                self.notify();
                self.update_timer();
            }

            last_update = Instant::now();
            thread::sleep(Duration::from_millis(500));
        }
    }

    fn reset(&mut self) {
        info!("Reset timer.");
        self.elapsed = Duration::ZERO;
        for item in self.timers.iter_mut() {
            item.reset();
        }
        self.update_timer();
    }

    fn notify(&self) {
        if let Some(timer) = self.next_timer.clone() {
            println!("Time to take a break!\x07"); // \x07 will ring the terminal bell

            if let Some(notification) = timer.notification {
                let description = {
                    if notification.descriptions.len() > 0 {
                        let rand_index = thread_rng().gen_range(0..notification.descriptions.len());
                        &notification.descriptions[rand_index]
                    } else {
                        "{} minutes elapsed."
                    }
                };
                let minutes_elapsed = self.elapsed.as_secs() / 60;
                let description = util::format_string(&description, &minutes_elapsed.to_string());
                util::show_notification(
                    notification.title,
                    description,
                    notification.timeout.unwrap_or(Duration::from_secs(10)),
                    timer.weight,
                );
            }

            if let Some(sound) = timer.sound {
                util::play_sound(sound);
            }

            if let Some(cmd) = timer.command {
                util::execute_command(cmd);
            }

            #[cfg(target_os = "linux")]
            if let Some(lock_conf) = timer.lock_screen {
                crate::lock_screen::start(lock_conf);
            }
        }
    }

    fn update_timer(&mut self) {
        // Determine the next timer
        for mut item in self.timers.iter_mut() {
            let mut in_timeout = false;
            let mut time_left = Duration::MAX;
            // Before timeout
            if let Some(timeout) = item.timer.timeout {
                if self.elapsed <= timeout {
                    time_left = timeout - self.elapsed;
                    in_timeout = true;
                }
            }
            if !in_timeout {
                // After timeout: every constant interval
                time_left = item.timer.interval
                    - Duration::from_secs(self.elapsed.as_secs() % item.timer.interval.as_secs())
            }
            item.time_left = time_left;
        }

        // Sort first by duration and then by type
        self.timers.sort_by(|a, b| {
            a.time_left
                .cmp(&b.time_left)
                .then(b.timer.weight.cmp(&a.timer.weight))
        });

        // First item is the next break
        if let Some(mut next) = self.timers.first_mut() {
            let next_duration = next.time_left;

            // The decline function, the iterval will be multiplied by 0.5 with a decline of 1.0
            let decline_mult = (1.0 / (1.0 + next.timer.decline)).powf(next.prompts as f64);
            let time_left = Duration::from_secs_f64(next_duration.as_secs_f64() * decline_mult);
            debug!(
                "Decline mult: {}, time: {:?}, prompt: {}",
                decline_mult, time_left, next.prompts
            );
            next.prompts += 1;

            println!("Next break over {:?}", time_left);

            self.time_left = self.elapsed + time_left;
            self.next_timer = Some(next.timer.clone());
        } else {
            error!("No timers found! Make sure to specify at least one in the config.");
        }
    }
}
