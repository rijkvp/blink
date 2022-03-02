use clap::Parser;
use notify_rust::Notification;
use rdev::listen;
use rodio::{source::Source, Decoder, OutputStream};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::BufReader,
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, RwLock,
    },
    thread::{self, sleep},
    time::{Duration, Instant, SystemTime},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Break {
    id: String,
    interval: Duration,
    timeout: Option<Duration>,
    weight: u8,
    resets_timer: bool,
    title: String,
    description: String,
    sound: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    breaks: Vec<Break>,
    activity_timeout: Duration,
    update_delay: Duration,
    sounds_folder: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            breaks: vec![Break {
                id: "test".to_string(),
                interval: Duration::from_secs(15),
                timeout: None,
                weight: 1,
                resets_timer: true,
                title: "Test".to_string(),
                description: "This is just for testing.".to_string(),
                sound: None,
            }],
            activity_timeout: Duration::from_secs(20),
            update_delay: Duration::from_secs(1),
            sounds_folder: None,
        }
    }
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Config file location
    #[clap(short)]
    config_path: Option<String>,
}

fn main() {
    let config = Config::default();

    Timer::new(config).start()
}

struct Timer {
    timer: Duration,
    time_left: Duration,
    curr_break: Break,
    cfg: Config,
    sender: Sender<Break>,
    receiver: Receiver<Break>,
}

impl Timer {
    pub fn new(config: Config) -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            timer: Duration::ZERO,
            time_left: Duration::ZERO,
            curr_break: config.breaks[0].clone(),
            cfg: config,
            sender,
            receiver,
        }
    }

    fn start(&mut self) {
        let last_activity = Arc::new(RwLock::new(Instant::now()));
        start_activity_tracking(last_activity.clone());

        let mut last_update = SystemTime::now();

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
        println!(
            "Break: {} - {} \x07",
            self.curr_break.title, self.curr_break.description
        ); // \x07 will ring the terminal bell

        show_notification(self.curr_break.clone(), self.sender.clone());

        if let (Some(sound_folder), Some(sound_filename)) =
            (&self.cfg.sounds_folder, &self.curr_break.sound)
        {
            let sound_file = PathBuf::from(&sound_folder).join(sound_filename);
            if sound_file.exists() {
                play_audio_file(sound_file.to_str().unwrap());
            } else {
                eprintln!("Sound file not found: {sound_file:?}");
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
            self.curr_break = next_break.clone().clone();
        } else {
            eprintln!("No break found!");
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
        thread::sleep(Duration::from_secs(5)); // Wait some time to finish
    });
}
