use clap::Parser;
use notify_rust::Notification;
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
    description: String,
    sound_file: Option<PathBuf>,
    #[serde(with = "duration_format")]
    interval: Duration,
    #[serde(default, with = "duration_format_option")]
    timeout: Option<Duration>,
    weight: u8,
    #[serde(default)]
    reset_timer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    #[serde(with = "duration_format")]
    activity_timeout: Duration,
    #[serde(with = "duration_format")]
    update_delay: Duration,
    sounds_folder: Option<PathBuf>,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            breaks: vec![
                Break {
                    interval: Duration::from_secs(60 * 15),
                    timeout: None,
                    weight: 0,
                    reset_timer: false,
                    title: "Blink".to_string(),
                    description: "Blink your eyes.".to_string(),
                    sound_file: Some(PathBuf::from("blink.ogg")),
                },
                Break {
                    interval: Duration::from_secs(60 * 30),
                    timeout: None,
                    weight: 1,
                    reset_timer: true,
                    title: "Small break".to_string(),
                    description: "Take a small break".to_string(),
                    sound_file: Some(PathBuf::from("break.ogg")),
                },
                Break {
                    interval: Duration::from_secs(5 * 60),
                    timeout: Some(Duration::from_secs(90 * 60)),
                    weight: 2,
                    reset_timer: true,
                    title: "Big break".to_string(),
                    description: "Take a big break".to_string(),
                    sound_file: Some(PathBuf::from("break.ogg")),
                },
            ],
            activity_timeout: Duration::from_secs(20),
            update_delay: Duration::from_secs(1),
            sounds_folder: None,
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
    let args = Args::parse();
    let config = {
        let config_path = args.config_path.unwrap_or({
            dirs::config_dir()
                .ok_or(Error::Fatal(
                    "No config directory found on your system.".to_string(),
                ))?
                .join("blink.toml")
        });

        if config_path.exists() {
            let config_str =
                fs::read_to_string(config_path).map_err(|e| Error::FileSystem(e.to_string()))?;
            toml::from_str(&config_str).map_err(|e| Error::Configfile(e.to_string()))?
        } else {
            println!("Config file not found. Generating default configuration..");
            let default_config = Config::default();
            let config_str = toml::to_string(&default_config)
                .map_err(|e| Error::Fatal(format!("Failed to serialize default config: {e}")))?;
            fs::write(config_path, &config_str).map_err(|e| Error::FileSystem(e.to_string()))?;
            default_config
        }
    };

    Timer::new(config).start();

    Ok(())
}

struct Timer {
    timer: Duration,
    time_left: Duration,
    curr_break: Option<Break>,
    cfg: Config,
    sender: Sender<Break>,
    receiver: Receiver<Break>,
}

impl Timer {
    pub fn new(config: Config) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            timer: Duration::ZERO,
            time_left: Duration::MAX,
            curr_break: None,
            cfg: config,
            sender,
            receiver,
        }
    }

    fn start(&mut self) {
        let last_activity = Arc::new(RwLock::new(Instant::now()));
        start_activity_tracking(last_activity.clone());

        let mut last_update = SystemTime::now();

        self.update_break();

        loop {
            let elapsed = last_update.elapsed().unwrap();

            let diff = last_activity.read().unwrap().elapsed();
            if diff < self.cfg.activity_timeout {
                self.timer += elapsed;
            }

            if self.timer >= self.time_left {
                self.start_break();
                self.update_break();
            }
            if let Ok(break_ref) = self.receiver.try_recv() {
                if break_ref.weight > 0 {
                    self.timer = Duration::ZERO;
                    println!("Reset timer");
                    self.update_break();
                }
            }

            last_update = SystemTime::now();
            sleep(self.cfg.update_delay);
        }
    }

    fn start_break(&mut self) {
        if let Some(break_info) = &self.curr_break {
            println!(
                "Break: {} - {} \x07",
                break_info.title, break_info.description
            ); // \x07 will ring the terminal bell

            show_notification(break_info.clone(), self.sender.clone());

            if let (Some(sound_folder), Some(sound_filename)) =
                (&self.cfg.sounds_folder, &break_info.sound_file)
            {
                let sound_file = PathBuf::from(&sound_folder).join(sound_filename);
                if sound_file.exists() {
                    play_audio_file(sound_file.to_str().unwrap());
                } else {
                    eprintln!("Sound file not found: {sound_file:?}");
                }
            }
        }
    }

    fn update_break(&mut self) {
        let mut breaks = Vec::new();

        for break_item in self.cfg.breaks.iter() {
            let mut in_timeout = false;
            let mut time_left = Duration::MAX;
            // Before timeout
            if let Some(timeout) = break_item.timeout {
                if self.timer <= timeout {
                    time_left = timeout - self.timer;
                    in_timeout = true;
                }
            }
            if !in_timeout {
                // After timeout: every constant interval
                time_left = break_item.interval
                    - Duration::from_secs(self.timer.as_secs() % break_item.interval.as_secs())
            }
            breaks.push((time_left, break_item));
        }

        // Sort first by time left and then by type
        breaks.sort_by(|a: &(Duration, &Break), b| a.0.cmp(&b.0).then(b.1.weight.cmp(&a.1.weight)));

        // First item is the next break
        if let Some((next_duration, next_break)) = breaks.first() {
            println!("Next break: {} over {:?}", next_break.title, next_duration);

            self.time_left = self.timer + *next_duration;
            self.curr_break = Some(next_break.clone().clone());
        } else {
            eprintln!("No breaks found. Specify at least one break in the config!");
            process::exit(1);
        }
    }
}

/// Displays a notification with the break info
fn show_notification(break_info: Break, callback: Sender<Break>) {
    thread::spawn(move || {
        let mut is_clicked = false;
        Notification::new()
            .appname("blink")
            .summary(&break_info.title)
            .body(&break_info.description)
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
    });
}

/// Starts a thread to keep track of the last input activities
fn start_activity_tracking(last_activity: Arc<RwLock<Instant>>) {
    thread::spawn(move || {
        if let Err(error) = listen(move |_event| {
            *last_activity.write().unwrap() = Instant::now();
        }) {
            println!("Error: {:?}", error)
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
