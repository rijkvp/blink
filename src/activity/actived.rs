use super::Activity;
use crate::{
    ActivityMessage, actived_socket_path, async_socket::SocketStream, get_unix_time, get_uptime,
};
use anyhow::Result;
use std::time::Duration;
use tokio::sync::mpsc;

pub async fn spawn(tx: mpsc::UnboundedSender<Activity>) -> Result<()> {
    let mut stream = SocketStream::connect(actived_socket_path()).await?;
    tokio::spawn(async move {
        loop {
            match stream.recv::<ActivityMessage>().await {
                Ok(msg) => {
                    // Convert on arrival,
                    // so a later NTP jump of the system clock does not shift it.
                    let Some(activity) = idle_since(msg.last_input) else {
                        continue;
                    };
                    if tx.send(activity).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    log::warn!("Activity daemon disconnected: {e:?}");
                    break;
                }
            }
        }
    });
    Ok(())
}

fn idle_since(last_input: u64) -> Option<Activity> {
    // The clock may have jumped since `actived` read it, so clamp the input between boot and now.
    let idle_for = Duration::from_secs(get_unix_time().saturating_sub(last_input));
    Some(Activity::IdleSince(get_uptime()?.saturating_sub(idle_for)))
}
