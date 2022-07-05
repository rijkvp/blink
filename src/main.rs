use clap::Parser;
use env_logger::Env;
use log::{debug, error, info, trace};
use notify_rust::Notification;
use rand::Rng;
use rdev::listen;
use rodio::{source::Source, Decoder, OutputStream};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::BufReader,
    path::PathBuf,
    process,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, RwLock,
    },
    thread::{self, sleep},
    time::{Duration, Instant, SystemTime},
};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Break {
    title: String,
    descriptions: Vec<String>,
    sound_file: Option<PathBuf>,
    #[serde(with = "duration_format")]
    interval: Duration,
    #[serde(
        default,
        with = "duration_format_option",
        skip_serializing_if = "Option::is_none"
    )]
    timeout: Option<Duration>,
    #[serde(default, skip_serializing_if = "is_default")]
    weight: u8,
    #[serde(default, skip_serializing_if = "is_default")]
    decay: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    #[serde(with = "duration_format")]
    update_delay: Duration,
    #[serde(with = "duration_format")]
    input_timeout: Duration,
    #[serde(with = "duration_format")]
    input_reset: Duration,
    #[serde(with = "duration_format")]
    timeout_reset: Duration,
    sounds_folder: Option<PathBuf>,
    time_descriptions: Vec<String>,
    #[serde(default, rename = "break")]
    breaks: Vec<Break>,
}

mod duration_format {
    use std::time::Duration;

    use serde::{de::Error, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&duration.as_secs().to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        let secs = str.parse().map_err(Error::custom)?;
        Ok(Duration::from_secs(secs))
    }
}

mod duration_format_option {
    use std::time::Duration;

    use serde::{de::Error, Deserializer, Serializer};

    use crate::duration_format;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(dur) => duration_format::serialize(dur, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match duration_format::deserialize(deserializer) {
            Ok(dur) => Ok(Some(dur)),
            Err(err) => Err(Error::custom(err)),
        }
    }
}

fn is_default<T: Default + PartialEq>(t: &T) -> bool {
    t == &T::default()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            breaks: vec![
                Break {
                    interval: Duration::from_secs(60 * 20),
                    timeout: None,
                    title: String::from("Micro break"),
                    descriptions: vec![
                        String::from("Don't forget to blink your eyes."),
                        String::from("Look away from the screen for a moment."),
                        String::from("Make sure you have a good posture."),
                    ],
                    ..Default::default()
                },
                Break {
                    interval: Duration::from_secs(60 * 30),
                    timeout: None,
                    weight: 1,
                    decay: 1.0,
                    title: String::from("Computer Break"),
                    descriptions: vec![
                        String::from("Get away from behind the screen!"),
                        String::from("Time to relax for a moment!"),
                    ],
                    ..Default::default()
                },
            ],
            time_descriptions: vec![
                String::from("Using the computer for {} minutes."),
                String::from("Staring at the screen for {} minutes."),
            ],
            sounds_folder: None,
            update_delay: Duration::from_secs(1),
            input_timeout: Duration::from_secs(30),
            input_reset: Duration::from_secs(300),
            timeout_reset: Duration::from_secs(200),
        }
    }
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Load config from custom path
    #[clap(short)]
    config_path: Option<PathBuf>,
}

#[derive(Debug)]
enum Error {
    FileSystem(String),
    Configfile(String),
    Fatal(String),
}

fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args = Args::parse();
    let config = {
        let config_path = args.config_path.unwrap_or({
            dirs::config_dir()
                .ok_or(Error::Fatal(
                    "No config directory found on your system.".to_string(),
                ))?
                .join("blink.toml")
        });
        trace!("Config path: {:?}", config_path);
        if config_path.exists() {
            let config_str =
                fs::read_to_string(config_path).map_err(|e| Error::FileSystem(e.to_string()))?;
            toml::from_str(&config_str).map_err(|e| Error::Configfile(e.to_string()))?
        } else {
            info!("Created default configuration at {config_path:?}");
            let default_config = Config::default();
            let config_str = toml::to_string(&default_config)
                .map_err(|e| Error::Fatal(format!("Failed to serialize default config: {e}")))?;
            fs::write(config_path, &config_str).map_err(|e| Error::FileSystem(e.to_string()))?;
            default_config
        }
    };
    trace!("Loaded config: {:?}", config);

    Timer::new(config).start();

    Ok(())
}

#[derive(Default, Clone)]
struct BreakState {
    time_left: Duration,
    prompts: u64,
    b: Break,
}

impl BreakState {
    fn new(b: Break) -> Self {
        Self {
            b: b,
            ..Default::default()
        }
    }
}

struct Timer {
    timer: Duration,
    time_left: Duration,
    curr_break: Option<Break>,
    state: Vec<BreakState>,
    cfg: Config,
    reset_tx: Sender<Break>,
    reset_rx: Receiver<Break>,
}

impl Timer {
    pub fn new(cfg: Config) -> Self {
        let (reset_tx, reset_rx) = mpsc::channel();
        let state = cfg
            .breaks
            .iter()
            .map(|b| BreakState::new(b.clone()))
            .collect();
        Self {
            timer: Duration::ZERO,
            time_left: Duration::MAX,
            curr_break: None,
            state,
            cfg,
            reset_tx,
            reset_rx,
        }
    }

    fn start(&mut self) {
        let last_input = Arc::new(RwLock::new(Instant::now()));
        start_input_tracking(last_input.clone());

        let mut last_update = SystemTime::now();

        self.update_break(false);

        // Main application loop
        loop {
            let mut time_elapsed = last_update.elapsed().unwrap();

            // Reset timer if the time since last update is greater than the timeout delay
            // This is probably caused by a system suspend
            if time_elapsed >= self.cfg.timeout_reset {
                info!("Resetting timer (timeout)");
                self.reset();
                time_elapsed = Duration::ZERO;
            }

            // Reset timer if time since last input is greater than the input_reset threshold
            let input_elapsed = last_input.read().unwrap().elapsed();
            if input_elapsed >= self.cfg.input_reset {
                info!("Resetting timer (input inactivity)");
                self.reset();
            }

            // Timer only runs if there was input in the last input_timeout
            if input_elapsed < self.cfg.input_timeout {
                self.timer += time_elapsed;
                trace!("TIMER: {:?}", self.timer);
            }

            // Start a break when the timer reaches the current break's interval
            if self.timer >= self.time_left {
                self.start_break(self.timer);
                self.update_break(false);
            }

            // Reset timer if break received through the channel form another thread
            if let Ok(break_ref) = self.reset_rx.try_recv() {
                // Breaks with a weight of 0 won't reset the timer
                if break_ref.weight > 0 {
                    self.reset();
                }
            }

            last_update = SystemTime::now();
            sleep(self.cfg.update_delay);
        }
    }

    fn reset(&mut self) {
        info!("Reset timer.");
        self.timer = Duration::ZERO;
        self.update_break(true);
    }

    fn start_break(&self, timer: Duration) {
        if let Some(break_info) = self.curr_break.clone() {
            let time_description = format_string(
                &self.cfg.time_descriptions
                    [rand::thread_rng().gen_range(0..self.cfg.time_descriptions.len())],
                &(timer.as_secs() / 60).to_string(),
            );
            let break_description = &break_info.descriptions
                [rand::thread_rng().gen_range(0..break_info.descriptions.len())];
            let description = format!("{}\n{}", break_description, time_description);
            info!("{}\n{}\x07", break_info.title, description); // \x07 will ring the terminal bell

            show_notification(
                break_info.clone(),
                break_info.title.clone(),
                description.clone(),
                self.reset_tx.clone(),
            );

            if let (Some(sound_folder), Some(sound_filename)) =
                (&self.cfg.sounds_folder, &break_info.sound_file)
            {
                let sound_file = PathBuf::from(&sound_folder).join(sound_filename);
                if sound_file.exists() {
                    play_audio_file(sound_file.to_str().unwrap());
                } else {
                    error!("Sound file not found: {sound_file:?}");
                }
            }
        }
    }

    fn update_break(&mut self, hide_msg: bool) {
        // let mut breaks: Vec<(Duration, BreakState)> = Vec::new();

        // Determine the next break
        for mut item in self.state.iter_mut() {
            let mut in_timeout = false;
            let mut time_left = Duration::MAX;
            // Before timeout
            if let Some(timeout) = item.b.timeout {
                if self.timer <= timeout {
                    time_left = timeout - self.timer;
                    in_timeout = true;
                }
            }
            if !in_timeout {
                // After timeout: every constant interval
                time_left = item.b.interval
                    - Duration::from_secs(self.timer.as_secs() % item.b.interval.as_secs())
            }
            item.time_left = time_left;
        }

        // Sort first by duration and then by type
        self.state.sort_by(|a, b| {
            a.time_left
                .cmp(&b.time_left)
                .then(b.b.weight.cmp(&a.b.weight))
        });

        // First item is the next break
        if let Some(mut next) = self.state.first_mut() {
            let next_duration = next.time_left;

            // The decay function, break with a decay of 1.0 will be halved after every prompt
            let decay_mult = (1.0 / (1.0 + next.b.decay)).powf(next.prompts as f64);
            let time_left = Duration::from_secs_f64(next_duration.as_secs_f64() * decay_mult);
            debug!(
                "Decay mult: {}, time: {:?}, prompt: {}",
                decay_mult, time_left, next.prompts
            );
            next.prompts += 1;

            if !hide_msg {
                info!("Next break: {} over {:?}", next.b.title, time_left);
            }

            self.time_left = self.timer + time_left;
            self.curr_break = Some(next.b.clone());
        } else {
            error!("No breaks found. Specify at least one break in the config!");
            process::exit(1);
        }
    }
}

/// Returns a string with the '{}' replaced with the input argument
fn format_string(source: &str, input: &str) -> String {
    let mut result = source.to_string();
    if let Some(index) = source.find("{}") {
        result.replace_range(index..index + 2, input);
    } else {
        error!("Invalid formating string '{}'.", source)
    }
    result
}

#[test]
fn format_string_test() {
    assert_eq!(format_string("Hello {}!", "world"), "Hello world!");
    assert_eq!(format_string("A & {}", "B"), "A & B");
}

/// Displays a notification with the break info
fn show_notification(
    break_info: Break,
    title: String,
    description: String,
    callback: Sender<Break>,
) {
    thread::spawn(move || {
        #[cfg(not(target_os = "linux"))]
        {
            Notification::new()
                .appname("blink")
                .summary(&title)
                .body(&description)
                .show()
                .unwrap();
        }
        #[cfg(target_os = "linux")]
        {
            let mut is_clicked = false;
            Notification::new()
                .appname("blink")
                .summary(&title)
                .body(&description)
                .action("default", "Complete")
                .show()
                .unwrap()
                .wait_for_action(|action| match action {
                    "default" => {
                        is_clicked = true;
                    }
                    _ => (),
                });
            if is_clicked {
                callback.send(break_info).unwrap();
            }
        }
    });
}

/// Starts a thread to keep track of the last input activities
fn start_input_tracking(last_input: Arc<RwLock<Instant>>) {
    thread::spawn(move || {
        if let Err(err) = listen(move |_event| {
            *last_input.write().unwrap() = Instant::now();
        }) {
            error!("Error while tracking input: {err:?}")
        }
    });
}

/// Loads and plays an audio file in a new thread
fn play_audio_file(sound_path: &str) {
    let path_clone = sound_path.to_owned().clone();
    thread::spawn(move || {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let break_sound =
            BufReader::new(File::open(&path_clone).expect("Failed to load break audio."));
        let audio_source = Decoder::new(break_sound).unwrap();
        stream_handle
            .play_raw(audio_source.convert_samples())
            .expect("Failed to play audio");
        thread::sleep(Duration::from_secs(20)); // Wait some time to make sure the sound finished playing
    });
}
