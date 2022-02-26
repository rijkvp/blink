use notify_rust::Notification;
use rdev::listen;
use rodio::{source::Source, Decoder, OutputStream};
use std::{
    env,
    fs::File,
    io::BufReader,
    path::PathBuf,
    sync::{mpsc::{self, Sender}, Arc, RwLock},
    thread::{self, sleep},
    time::{Duration, Instant, SystemTime},
};

#[derive(Debug, Clone, PartialEq, PartialOrd)]
enum BreakType {
    Blink = 0,
    Short = 1,
    Normal = 2,
    Forced = 3,
}

const INTERVAL_BREAKS: [(u64, BreakType); 2] = [(30*60, BreakType::Short), (15*60, BreakType::Blink)];
const CONSTANT_BREAKS: [(u64, BreakType); 2] = [(120*60, BreakType::Forced), (60*60, BreakType::Normal)];
const CONSTANT_BREAK_INTERVAL: u64 = 5;
const ACTIVITY_TIMEOUT: u64 = 30;

fn main() {
    let args: Vec<String> = env::args().collect();

    let sound_path = args.get(1).to_owned();

    let last_activity = Arc::new(RwLock::new(Instant::now()));
    start_activity_tracking(last_activity.clone());

    let mut last_update = SystemTime::now();
    let mut timer = Duration::ZERO;
    let (tx, rx) = mpsc::channel();
    loop {
        sleep(Duration::from_secs(1));
        let elapsed = last_update.elapsed().unwrap();

        let diff = last_activity.read().unwrap().elapsed();
        if diff < Duration::from_secs(ACTIVITY_TIMEOUT) {
            timer += elapsed;
        }

        if let Some(break_type) = check_break(&timer) {
            println!("Break!! ({break_type:?}) \x07"); // \x07 will ring the terminal bell

            show_notification(tx.clone(), break_type.clone());

            if let Some(sound_folder) = sound_path {
                let sound_file = PathBuf::from(&sound_folder)
                    .join(format!("{break_type:?}").to_lowercase() + ".ogg");
                if sound_file.exists() {
                    play_audio_file(sound_file.to_str().unwrap());
                } else {
                    eprintln!("Sound file not found: {sound_file:?}");
                }
            }
        }

        if let Ok(break_type) = rx.try_recv() {
            if break_type > BreakType::Blink {
                timer = Duration::ZERO;
            }
        }

        last_update = SystemTime::now();
    }
}

fn check_break(timer: &Duration) -> Option<BreakType> {
    // Constant breaks
    for (timeout, break_type) in CONSTANT_BREAKS {
        if timer.as_secs() >= timeout {
            if timer.as_secs() % CONSTANT_BREAK_INTERVAL == 0 {
                return Some(break_type);
            }
        }
    }

    // Interval breaks
    for (interval, break_type) in INTERVAL_BREAKS {
        if timer.as_secs() % interval == 0 {
            return Some(break_type);
        }
    }

    None
}

fn show_notification(sender: Sender<BreakType>, break_type: BreakType) {
    thread::spawn(move || {
        let mut is_clicked = false;
        Notification::new()
            .appname("blink")
            .summary(&format!("{break_type:?} break"))
            .body("Time to take a break!")
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
            sender.send(break_type).unwrap();
        }
    });
}

// Keep track of the last input activities
fn start_activity_tracking(last_activity: Arc<RwLock<Instant>>) {
    thread::spawn(move || {
        if let Err(error) = listen(move |_event| {
            *last_activity.write().unwrap() = Instant::now();
        }) {
            println!("Error: {:?}", error)
        }
    });
}

/// Load and plays an audio file in a new thread
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
