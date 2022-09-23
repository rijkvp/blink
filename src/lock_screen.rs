use log::{debug, error};
use std::{
    thread,
    time::{Duration, Instant},
};
use xcb::x;

use crate::config::LockScreen;

pub fn start(config: LockScreen) {
    thread::spawn(move || {
        debug!("Timeout {:?} before lock screen", config.timeout);
        thread::sleep(config.timeout);
        debug!("Opening lock screen");
        if let Err(e) = open(config.duration, config.escape) {
            error!("error while opening overlay: {e}");
        }
    });
}

fn open(duration: Duration, allow_quitting: bool) -> xcb::Result<()> {
    let start = Instant::now();

    let (conn, screen_num) = xcb::Connection::connect(None).unwrap();
    let setup = conn.get_setup();
    let screen = setup.roots().nth(screen_num as usize).unwrap();
    let width = screen.width_in_pixels();
    let height = screen.height_in_pixels();

    let window: x::Window = conn.generate_id();

    conn.send_request(&x::CreateWindow {
        depth: x::COPY_FROM_PARENT as u8,
        wid: window,
        parent: screen.root(),
        x: 0,
        y: 0,
        width,
        height,
        border_width: 0,
        class: x::WindowClass::InputOutput,
        visual: screen.root_visual(),
        value_list: &[
            x::Cw::BackPixel(screen.black_pixel()),
            x::Cw::OverrideRedirect(true),
            x::Cw::EventMask(x::EventMask::EXPOSURE | x::EventMask::KEY_PRESS),
        ],
    });

    conn.send_request(&x::MapWindow { window });
    conn.send_request(&x::SetInputFocus {
        revert_to: x::InputFocus::PointerRoot,
        focus: window,
        time: x::CURRENT_TIME,
    });

    conn.flush()?;

    // Event thead
    thread::spawn(move || loop {
        let event = match conn.poll_for_event() {
            Err(xcb::Error::Connection(err)) => {
                error!("unexpected I/O error: {}", err);
                break;
            }
            Err(xcb::Error::Protocol(err)) => {
                error!("unexpected protocol error: {:#?}", err);
                break;
            }
            Ok(event) => event,
        };
        if let Some(xcb::Event::X(x::Event::KeyPress(ev))) = event {
            debug!("Key '{}' pressed", ev.detail());
            if ev.detail() == 24 && allow_quitting {
                debug!("Quit overlay");
                break;
            }
        }
        if start.elapsed() > duration {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    });

    Ok(())
}
