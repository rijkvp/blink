mod actived;
#[cfg(feature = "wayland")]
mod wayland;

use crate::{
    config::{InputBackend, InputTracking},
    get_uptime,
};
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// The user is currently active (only reported by edge-triggered backends).
    Active,
    /// No input since this uptime.
    /// Does not jump when NTP sets the system clock (unlike unix time).
    IdleSince(Duration),
}

impl Activity {
    pub fn idle_for(&self, now_uptime: Duration) -> Duration {
        match self {
            Activity::Active => Duration::ZERO,
            Activity::IdleSince(since) => now_uptime.saturating_sub(*since),
        }
    }
}

/// Starts the configured input tracking backend.
///
/// Setup errors (e.g. the backend not being available) are returned directly.
/// If the backend fails later on, it logs the reason and closes the channel.
pub async fn spawn_backend(tracking: &InputTracking) -> Result<mpsc::UnboundedReceiver<Activity>> {
    get_uptime().context("input tracking needs `/proc/uptime`")?;
    let (tx, rx) = mpsc::unbounded_channel();
    match tracking.backend {
        InputBackend::Actived => actived::spawn(tx).await?,
        #[cfg(feature = "wayland")]
        InputBackend::Wayland => wayland::spawn(tx, tracking)?,
        #[cfg(not(feature = "wayland"))]
        InputBackend::Wayland => {
            anyhow::bail!("blinkd was built without the `wayland` feature")
        }
    }
    Ok(rx)
}
