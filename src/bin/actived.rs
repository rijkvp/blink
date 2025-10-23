/// actived is a daemon that determines if a user is 'active' or not by listening to input events.
/// It is intended to be ran seperately since it needs root permissions to access input devices.
use anyhow::{Context, Result, bail};
use blink_timer::{
    ActivityMessage,
    async_socket::{SocketServer, SocketStream},
};
use evdev::{Device, EventType};
use std::{
    process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::broadcast;
use tokio_stream::{StreamExt, StreamMap};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    // Broadcast channel for distributing events to all clients
    let (broadcast_tx, _) = broadcast::channel::<u64>(100);

    // Last input timestamp
    let last_input = Arc::new(AtomicU64::new(blink_timer::get_unix_time()));

    // Start input listener
    tokio::spawn({
        let last_input = last_input.clone();
        let broadcast_tx = broadcast_tx.clone();

        async move {
            if let Err(e) = run_input_listener(broadcast_tx, last_input).await {
                log::error!("event listener failed: {e}");
                process::exit(1);
            }
        }
    });

    let mut socket_server = SocketServer::create(blink_timer::actived_socket_path(), true)
        .await
        .context("failed to create socket server")?;
    log::info!("listening for client connections");

    // Accept and handle clients
    loop {
        let stream = socket_server.accept_client().await?;
        let broadcast_rx = broadcast_tx.subscribe();
        let last_input = last_input.clone();

        // Spawn a new task for each client
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, broadcast_rx, last_input).await {
                log::error!("client handler error: {e:?}");
            }
        });
    }
}

async fn handle_client(
    mut stream: SocketStream,
    mut broadcast_rx: broadcast::Receiver<u64>,
    last_input: Arc<AtomicU64>,
) -> Result<()> {
    // send initial value to the client
    let timestamp = last_input.load(Ordering::Relaxed);
    stream
        .send(&ActivityMessage {
            last_active: timestamp,
        })
        .await?;

    // Listen for events and forward them to the client
    while let Ok(timestamp) = broadcast_rx.recv().await {
        stream
            .send(&ActivityMessage {
                last_active: timestamp,
            })
            .await?;
    }

    Ok(())
}

enum InputEvent {
    Keyboard,
    Mouse,
}

async fn run_input_listener(
    broadcast_tx: broadcast::Sender<u64>,
    last_input: Arc<AtomicU64>,
) -> Result<()> {
    let devices: Vec<Device> = evdev::enumerate()
        .map(|(_, device)| device)
        .filter(|d| {
            // Filter on keyboard, mouse & touchscreen devices
            let supported = d.supported_events();
            supported.contains(EventType::KEY)
                || supported.contains(EventType::RELATIVE)
                || supported.contains(EventType::ABSOLUTE)
        })
        .collect();
    if devices.is_empty() {
        bail!("no input devices found! are you running as root?")
    }
    log::info!("listening for events on {} input devices", devices.len());
    let mut streams = StreamMap::new();
    for (n, device) in devices.into_iter().enumerate() {
        streams.insert(n, device.into_event_stream()?);
    }
    while let Some((_, Ok(event))) = streams.next().await {
        let event = match event.event_type() {
            EventType::KEY => Some(InputEvent::Keyboard),
            EventType::RELATIVE | EventType::ABSOLUTE => Some(InputEvent::Mouse),
            _ => None,
        };
        if event.is_some() {
            let timestamp = blink_timer::get_unix_time();
            log::debug!("input event received at {timestamp}");

            last_input.store(timestamp, Ordering::Relaxed);
            let _ = broadcast_tx.send(timestamp);
        }
    }
    Ok(())
}
