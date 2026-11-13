use super::Activity;
use crate::{config::InputTracking, get_uptime};
use anyhow::{Context, Result};
use std::{thread, time::Duration};
use tokio::sync::mpsc;
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{wl_registry::WlRegistry, wl_seat::WlSeat},
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};

struct State {
    tx: mpsc::UnboundedSender<Activity>,
    idle_timeout: Duration,
}

pub fn spawn(tx: mpsc::UnboundedSender<Activity>, tracking: &InputTracking) -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("failed to connect to the Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, mut queue) =
        registry_queue_init::<State>(&conn).context("failed to list Wayland globals")?;
    let qh = queue.handle();
    let seat: WlSeat = globals
        .bind(&qh, 1..=1, ())
        .context("failed to bind wl_seat")?;
    let notifier: ExtIdleNotifierV1 = globals
        .bind(&qh, 1..=2, ())
        .context("compositor does not support ext-idle-notify-v1")?;

    // Until the compositor reports idleness, the daemon sees the user as active.
    // The compositor must therefore report before either threshold is reached,
    // so the timeout is the smaller of the two, even if `reset_after < pause_after`.
    let idle_timeout = tracking.pause_after.min(tracking.reset_after);
    let timeout = u32::try_from(idle_timeout.as_millis()).unwrap_or(u32::MAX);
    if notifier.version() >= 2 {
        log::info!("Tracking input idleness via the Wayland compositor");
        notifier.get_input_idle_notification(timeout, &seat, &qh, ());
    } else {
        log::warn!(
            "Compositor only supports ext-idle-notify-v1 version 1: \
             idle inhibitors (e.g. video playback) will count as activity"
        );
        notifier.get_idle_notification(timeout, &seat, &qh, ());
    }

    let mut state = State { tx, idle_timeout };
    thread::spawn(move || {
        // Only checked after the next event, so the thread may outlive the receiver until then
        while !state.tx.is_closed() {
            if let Err(e) = queue.blocking_dispatch(&mut state) {
                log::warn!("Wayland connection lost: {e}");
                break;
            }
        }
    });
    Ok(())
}

impl Dispatch<ExtIdleNotificationV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let activity = match event {
            // The compositor sends this once `idle_timeout` has passed without input.
            // Use it to get the time of the last input.
            // Then we can count just like actived does.
            ext_idle_notification_v1::Event::Idled => {
                let Some(now_uptime) = get_uptime() else {
                    return;
                };
                Activity::IdleSince(now_uptime.saturating_sub(state.idle_timeout))
            }
            ext_idle_notification_v1::Event::Resumed => Activity::Active,
            _ => return,
        };
        // A closed channel is handled by the dispatch loop
        let _ = state.tx.send(activity);
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

delegate_noop!(State: ignore WlSeat);
delegate_noop!(State: ExtIdleNotifierV1);
