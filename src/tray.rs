//! Optional status-bar menu (`--features tray`). Menu events reach GPUI's main thread through
//! a channel. On macOS, `run` hides the Dock icon before launch.

use gpui_kit::{ActivationPolicy, App, AsyncApp, QuitMode, base::async_util::unbounded};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

const SHOW: &str = "show";
const QUIT: &str = "quit";

pub fn install(cx: &mut App) {
    // Before the icon exists: an event that arrives first leaves muda without a handler for good.
    let (sender, receiver) = unbounded::<MenuEvent>();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| drop(sender.try_send(event))));

    if let Err(err) = create_icon(cx) {
        eprintln!("{}: no tray icon: {err}", crate::APP_NAME);
        // Back to a Dock icon and menu bar; `run` activates the app next.
        cx.set_activation_policy(ActivationPolicy::Regular);
        return;
    }
    // Only with an icon: without one, closing the window must still quit on Linux.
    cx.set_quit_mode(QuitMode::Explicit);

    cx.spawn(async move |cx: &mut AsyncApp| {
        while let Ok(event) = receiver.recv().await {
            cx.update(|cx| {
                if event.id == QUIT {
                    cx.quit();
                } else if event.id == SHOW {
                    crate::show_main_window(cx);
                    cx.activate(true);
                }
            });
        }
    })
    .detach();
}

fn build() -> Result<TrayIcon, String> {
    let menu = Menu::new();
    menu.append_items(&[
        &MenuItem::with_id(SHOW, format!("Show {}", crate::APP_NAME), true, None),
        &MenuItem::with_id(QUIT, format!("Quit {}", crate::APP_NAME), true, None),
    ])
    .map_err(|err| err.to_string())?;
    TrayIconBuilder::new()
        .with_tooltip(crate::APP_NAME)
        .with_icon(icon())
        .with_menu(Box::new(menu))
        .build()
        .map_err(|err| err.to_string())
}

/// macOS: the icon lives on the main thread (GPUI's AppKit loop), kept alive in a global.
#[cfg(not(target_os = "linux"))]
fn create_icon(cx: &mut App) -> Result<(), String> {
    struct Tray(#[allow(dead_code)] TrayIcon);
    impl gpui_kit::Global for Tray {}
    cx.set_global(Tray(build()?));
    Ok(())
}

/// Linux: tray-icon needs a GTK event loop on the thread that owns the icon, and GPUI
/// runs its own loop, so the icon gets a GTK thread of its own.
#[cfg(target_os = "linux")]
fn create_icon(_: &mut App) -> Result<(), String> {
    let (ready, started) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let icon = gtk::init()
            .map_err(|err| err.to_string())
            .and_then(|()| build());
        let status = icon.as_ref().map(|_| ()).map_err(String::clone);
        let running = status.is_ok();
        drop(ready.send(status));
        if running {
            gtk::main();
        }
    });
    started.recv().map_err(|err| err.to_string())?
}

/// A 32×32 rounded square in the accent colour. To use your own artwork, decode it to RGBA
/// (e.g. with the `image` crate) and pass it to `Icon::from_rgba`.
fn icon() -> Icon {
    const SIZE: u32 = 32;
    const RADIUS: f32 = 6.0;
    let (lo, hi) = (2.0, SIZE as f32 - 2.0);
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (x, y) = (x as f32, y as f32);
            let (dx, dy) = (
                x - x.clamp(lo + RADIUS, hi - RADIUS),
                y - y.clamp(lo + RADIUS, hi - RADIUS),
            );
            let inside = (lo..=hi).contains(&x)
                && (lo..=hi).contains(&y)
                && dx * dx + dy * dy <= RADIUS * RADIUS + 0.5;
            rgba.extend_from_slice(if inside {
                &[0x00, 0x64, 0xE1, 0xFF]
            } else {
                &[0, 0, 0, 0]
            });
        }
    }
    Icon::from_rgba(rgba, SIZE, SIZE).expect("a 32x32 RGBA buffer is a valid icon")
}
