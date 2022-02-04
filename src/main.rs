use rdev::listen;
use rodio::{source::Source, Decoder, OutputStream};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::sync::{Arc, RwLock};
use std::thread::{self, sleep};
use std::time::Instant;
use std::time::{Duration, SystemTime};

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let break_interval = {
        if let Some(string) = args.get(1) {
            Duration::from_secs(string.parse().expect("Invalid input for break interval."))
        } else {
            Duration::from_secs(60 * 25)
        }
    };
    let sound_path = args.get(2).to_owned();
    let activity_timeout = {
        if let Some(string) = args.get(3) {
            Duration::from_secs(string.parse().expect("Invalid input for activity timeout."))
        } else {
            Duration::from_secs(30)
        }
    };

    let last_activity = Arc::new(RwLock::new(Instant::now()));
    start_activity_thread(last_activity.clone());

    let mut last_update = SystemTime::now();
    let mut timer = Duration::new(0, 0);
    println!(
        "Starting timer with {}s break interval, sound path '{:?}', {}s activity timeout.",
        break_interval.as_secs(),
        sound_path,
        activity_timeout.as_secs(),
    );
    loop {
        sleep(Duration::from_secs(1));
        let elapsed = last_update.elapsed().unwrap();

        let diff = last_activity.read().unwrap().elapsed();
        if diff < activity_timeout {
            timer += elapsed;
        }

        if timer > break_interval {
            // \x07 will ring the terminal bell (if enabled)
            println!("Time to take a break! \x07");
            timer = Duration::new(0, 0);
            if let Some(path) = sound_path {
                start_audio_thread(&path.clone());
            }
        }
        last_update = SystemTime::now();
    }
}

fn start_activity_thread(last_activity: Arc<RwLock<Instant>>) {
    thread::spawn(move || {
        if let Err(error) = listen(move |_event| {
            *last_activity.write().unwrap() = Instant::now();
        }) {
            println!("Error: {:?}", error)
        }
    });
}

fn start_audio_thread(sound_path: &str) {
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
