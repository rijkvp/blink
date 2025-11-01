pub mod async_socket;
pub mod config;
pub mod util;

use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Write},
    path::PathBuf,
    time::{Duration, SystemTime},
};

pub const APP_NAME: &str = "blink";
pub const ACTIVED_NAME: &str = "actived";

pub fn socket_path() -> PathBuf {
    dirs::runtime_dir()
        .expect("No runtime directory found!")
        .join(APP_NAME)
        .join(APP_NAME)
        .with_extension("sock")
}

pub fn actived_socket_path() -> PathBuf {
    PathBuf::from("/run")
        .join(APP_NAME)
        .join(ACTIVED_NAME)
        .with_extension("sock")
}

pub fn get_unix_time() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityMessage {
    pub last_input: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcRequest {
    Status,
    Toggle,
    Reset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcResponse {
    Ok,
    Status(Status),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    elapsed: Duration,
    next_timer: Duration,
}

impl Status {
    pub fn new(elapsed: Duration, next_timer: Duration) -> Self {
        Self {
            elapsed,
            next_timer,
        }
    }
}

impl Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_duration(f, self.elapsed)?;
        f.write_char('/')?;
        format_duration(f, self.next_timer)?;
        Ok(())
    }
}

pub trait DurationExt {
    fn display(&self) -> DurationDisplay;
}

impl DurationExt for Duration {
    fn display(&self) -> DurationDisplay {
        DurationDisplay(*self)
    }
}

pub struct DurationDisplay(Duration);

impl Display for DurationDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_duration(f, self.0)
    }
}

fn format_duration(f: &mut fmt::Formatter, duration: Duration) -> fmt::Result {
    let mut secs = duration.as_secs_f64().round() as u64;
    let days = secs / 86_400;
    secs %= 86_400;
    let hours = secs / 3_600;
    secs %= 3_600;
    let minutes = secs / 60;
    secs %= 60;
    if days > 0 {
        write!(f, "{}d ", days)?;
    }
    if hours > 0 {
        write!(f, "{:02}:", hours)?;
    }
    write!(f, "{:02}:{:02}", minutes, secs)
}
