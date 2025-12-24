use notify_rust::{Notification, Timeout, Urgency};
use std::{
    fs::File,
    io::BufReader,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
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
pub fn show_notification(title: String, description: String, timeout: Option<u32>) {
    thread::spawn(move || {
        if let Err(e) = Notification::new()
            .appname("blink")
            .summary(&title)
            .body(&description)
            .timeout(if let Some(timeout) = timeout {
                if timeout == 0 {
                    Timeout::Never
                } else {
                    Timeout::Milliseconds(timeout * 1000)
                }
            } else {
                Timeout::Default
            })
            .urgency(if timeout == Some(0) {
                Urgency::Critical
            } else {
                Urgency::Normal
            })
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
pub fn play_sound(path: PathBuf) {
    thread::spawn(move || {
        let Ok(mut stream_handle) = rodio::OutputStreamBuilder::open_default_stream() else {
            log::error!("Failed to open default audio stream");
            return;
        };
        stream_handle.log_on_drop(false);

        let Ok(file) = File::open(&path) else {
            log::error!("Failed to open audio file '{}'", path.display());
            return;
        };
        let file = BufReader::new(file);

        let Ok(sink) = rodio::play(stream_handle.mixer(), file) else {
            log::error!("Failed to play audio");
            return;
        };
        sink.sleep_until_end();
    });
}
