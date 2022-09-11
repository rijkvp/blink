use crate::config::Sound;
use log::{error, info};
use notify_rust::Notification;
use rodio::{Decoder, OutputStream, Source};
use std::{
    fs::File,
    io::BufReader,
    process::{Command, Stdio},
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

/// Returns a string with the '{}' replaced with the input argument
pub fn format_string(source: &str, input: &str) -> String {
    let mut result = source.to_string();
    if let Some(index) = source.find("{}") {
        result.replace_range(index..index + 2, input);
    }
    result
}

#[test]
fn format_string_test() {
    assert_eq!(format_string("Hello {}!", "world"), "Hello world!");
    assert_eq!(format_string("A & {}", "B"), "A & B");
}

/// Displays a notification with the break info
pub fn show_notification(title: String, description: String, timeout: Duration, urgency: u8) {
    thread::spawn(move || {
        let urgency = match urgency {
            0 => notify_rust::Urgency::Low,
            1 => notify_rust::Urgency::Normal,
            2.. => notify_rust::Urgency::Critical,
        };
        if let Err(e) = Notification::new()
            .appname("blink")
            .summary(&title)
            .body(&description)
            .action("default", "Complete")
            .urgency(urgency)
            .timeout(timeout.as_millis() as i32)
            .show()
        {
            error!("Failed to show notification: {e}");
        }
    });
}

/// Starts a thread to keep track of the last input activities
pub fn start_input_tracking(last_input: Arc<RwLock<Instant>>) {
    thread::spawn(move || {
        if let Err(err) = rdev::listen(move |_event| {
            *last_input.write().unwrap() = Instant::now();
        }) {
            error!("Error while tracking input: {err:?}")
        }
    });
}

/// Runs a command in a new thread, output is logged when unsuccesful
pub fn execute_command(command: String) {
    thread::spawn(move || {
        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to execute process")
            .wait_with_output()
            .expect("failed to wait");
        if !output.status.success() {
            error!("command '{}' failed ({})", &command, output.status);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stdout = stdout.trim();
            if stdout.len() > 0 {
                error!("stdout: {}", stdout);
            }
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stderr = stderr.trim();
            if stderr.len() > 0 {
                error!("stderr: {}", stderr);
            }
        } else {
            info!("command '{}' finished succesfully", &command);
        }
    });
}

/// Loads and plays an audio file in a new thread
pub fn play_sound(sound: Sound) {
    thread::spawn(move || {
        let (_stream, stream_handle) = OutputStream::try_default().unwrap();
        let break_sound =
            BufReader::new(File::open(&sound.path).expect("Failed to load break audio."));
        let audio_source = Decoder::new(break_sound).unwrap();
        stream_handle
            .play_raw(audio_source.convert_samples())
            .expect("Failed to play audio");
        thread::sleep(sound.duration);
    });
}
