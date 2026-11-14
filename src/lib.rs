pub mod async_socket;
pub mod config;
pub mod util;

use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Write},
    path::PathBuf,
    time::Duration,
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

/// Returns the time since boot, including time spent suspended.
pub fn get_uptime() -> Duration {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // panic on failure, requires Linux > 2.6.39.
    let ret = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
    assert_eq!(
        ret,
        0,
        "clock_gettime(CLOCK_BOOTTIME) failed: {}",
        std::io::Error::last_os_error()
    );
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityMessage {
    /// The [get_uptime] at the last input.
    pub last_input: Duration,
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
