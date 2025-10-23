use crate::config::Sound;
use notify_rust::Notification;
use std::{
    fs::File,
    io::BufReader,
    process::{Command, Stdio},
    thread,
    time::Duration,
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
            .urgency(urgency)
            .timeout(timeout.as_millis() as i32)
            .show()
        {
            log::error!("Failed to show notification: {e}");
        }
    });
}

/// Runs a command in a new thread, output is logged when unsuccesful
pub fn exec_command(command: String) {
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
            log::error!("Command '{}' failed ({})", &command, output.status);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stdout = stdout.trim();
            if !stdout.is_empty() {
                log::error!("stdout: {}", stdout);
            }
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let stderr = stderr.trim();
            if !stderr.is_empty() {
                log::error!("stderr: {}", stderr);
            }
        } else {
            log::info!("Command '{}' finished succesfully", &command);
        }
    });
}

/// Loads and plays an audio file in a new thread
pub fn play_sound(sound: Sound) {
    thread::spawn(move || {
        let stream_handle = rodio::OutputStreamBuilder::open_default_stream()
            .expect("failed open default audio stream");
        let file = BufReader::new(File::open(&sound.path).expect("failed to open audio file"));
        let sink = rodio::play(stream_handle.mixer(), file).expect("failed to play audio");
        sink.sleep_until_end();
    });
}
