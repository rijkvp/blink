/// actived is a daemon that determines if a user is 'active' or not by listening to input events.
/// It is intended to be ran seperately since it needs root permissions to access input devices.
use anyhow::{Context, Result, bail};
use blink_timer::{
    ActivityMessage, DurationExt,
    async_socket::{SocketServer, SocketStream},
    get_uptime,
};
use evdev::{Device, EventType};
use std::{
    process,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::broadcast;
use tokio::task::JoinSet;

// Minimum time between emitting events
const EVENT_COOLDOWN: Duration = Duration::from_millis(500);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_default_env()
        .init();

    // Broadcast channel for distributing events to all clients
    let (broadcast_tx, _) = broadcast::channel::<Duration>(100);

    let last_input_nanos = Arc::new(AtomicU64::new(get_uptime().as_nanos() as u64));

    // Start input listener
    tokio::spawn({
        let last_input_nanos = last_input_nanos.clone();
        let broadcast_tx = broadcast_tx.clone();

        async move {
            if let Err(e) = run_input_listener(broadcast_tx, last_input_nanos).await {
                log::error!("Event listener failed: {e}");
                process::exit(1);
            }
        }
    });

    let mut socket_server = SocketServer::create(blink_timer::actived_socket_path(), true)
        .await
        .context("Failed to create socket server")?;
    log::info!("listening for client connections");

    // Accept and handle clients
    loop {
        let stream = socket_server.accept_client().await?;
        let broadcast_rx = broadcast_tx.subscribe();
        let last_input_nanos = last_input_nanos.clone();

        // Spawn a new task for each client
        tokio::spawn(async move {
            if let Err(e) = handle_client(stream, broadcast_rx, last_input_nanos).await {
                log::error!("Client handler error: {e:?}");
            }
        });
    }
}

async fn handle_client(
    mut stream: SocketStream,
    mut broadcast_rx: broadcast::Receiver<Duration>,
    last_input_nanos: Arc<AtomicU64>,
) -> Result<()> {
    // send initial value to the client
    let last_input = Duration::from_nanos(last_input_nanos.load(Ordering::Relaxed));
    stream.send(&ActivityMessage { last_input }).await?;

    // Listen for events and forward them to the client
    while let Ok(last_input) = broadcast_rx.recv().await {
        stream.send(&ActivityMessage { last_input }).await?;
    }

    Ok(())
}

#[derive(Debug)]
enum InputEvent {
    Keyboard,
    Mouse,
}

async fn run_input_listener(
    broadcast_tx: broadcast::Sender<Duration>,
    last_input_nanos: Arc<AtomicU64>,
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
        bail!("No input devices found! are you running as root?")
    }
    log::info!("Listening for events on {} input devices", devices.len());

    let mut set = JoinSet::new();
    for device in devices {
        let broadcast_tx = broadcast_tx.clone();
        let last_input_nanos = last_input_nanos.clone();
        set.spawn_blocking(move || run_device_listener(device, broadcast_tx, last_input_nanos));
    }

    if let Some(result) = set.join_next().await {
        return result?;
    }
    Ok(())
}

fn run_device_listener(
    mut device: Device,
    broadcast_tx: broadcast::Sender<Duration>,
    last_input_nanos: Arc<AtomicU64>,
) -> Result<()> {
    let mut last_emit = Instant::now();

    loop {
        for event in device.fetch_events().context("Failed to fetch events")? {
            let event = match event.event_type() {
                EventType::KEY => InputEvent::Keyboard,
                EventType::RELATIVE | EventType::ABSOLUTE => InputEvent::Mouse,
                _ => continue,
            };

            if last_emit.elapsed() < EVENT_COOLDOWN {
                continue;
            }
            last_emit = Instant::now();

            let uptime = get_uptime();
            log::debug!("Input {event:?} received at uptime {}", uptime.display());

            last_input_nanos.store(uptime.as_nanos() as u64, Ordering::Relaxed);
            let _ = broadcast_tx.send(uptime);
        }
    }
}
