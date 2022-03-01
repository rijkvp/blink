use notify_rust::Notification;
use rdev::listen;
use rodio::{source::Source, Decoder, OutputStream};
use std::{
    env,
    fs::File,
    io::BufReader,
    path::PathBuf,
    sync::{
        mpsc::{self, Sender},
        Arc, RwLock,
    },
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
const UPDATE_DELAY: u64 = 5;

fn main() {
    let args: Vec<String> = env::args().collect();

    let sound_path = args.get(1).to_owned();

    let last_activity = Arc::new(RwLock::new(Instant::now()));
    start_activity_tracking(last_activity.clone());

    let mut last_update = SystemTime::now();
    let mut timer = Duration::ZERO;
    let mut time_left = 0;
    let mut break_type = BreakType::Blink;
    let (tx, rx) = mpsc::channel();
    loop {
        let elapsed = last_update.elapsed().unwrap();

        let diff = last_activity.read().unwrap().elapsed();
        if diff < Duration::from_secs(ACTIVITY_TIMEOUT) {
            timer += elapsed;
        }
        println!("Timer: {} ({})", timer.as_secs(), timer.as_secs_f32());
        if timer.as_secs() >= time_left {
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

            if let Some((new_time_left, new_break_type)) = next_break(&timer) {
                time_left = new_time_left;
                break_type = new_break_type;
                println!("Next break: {break_type:?} over {time_left}s");
            }
        }
        if let Ok(break_type) = rx.try_recv() {
            if break_type > BreakType::Blink {
                timer = Duration::ZERO;
                println!("Reset timer");
            }
        }

        last_update = SystemTime::now();
        sleep(Duration::from_secs(UPDATE_DELAY));
    }
}

fn next_break(time: &Duration) -> Option<(u64, BreakType)> {
    let mut breaks = Vec::new();

    // Constant breaks
    for (timeout, break_type) in CONSTANT_BREAKS {
        let time_left = {
            // Before timeout
            if time.as_secs() <= timeout {
                timeout - time.as_secs()
            }
            // After timeout: every constant interval
            else {
                CONSTANT_BREAK_INTERVAL - (time.as_secs() % CONSTANT_BREAK_INTERVAL)
            }
        };
        breaks.push((time_left, break_type));
    }
    // Interval breaks
    for (interval, break_type) in INTERVAL_BREAKS {
        let time_left = interval - (time.as_secs() % interval);
        breaks.push((time_left, break_type));
    }

    // Sort first by time left and then by type
    breaks.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.partial_cmp(&a.1).unwrap()));

    println!("BREAKS: {breaks:#?}");

    // First item is the next break
    if let Some(next) = breaks.first() {
        return Some(next.clone());
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
