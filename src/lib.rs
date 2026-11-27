//! {{app_name}}. A library, so the binary and `examples/shot.rs` share it.

mod home;
pub mod launch;
#[cfg(target_os = "macos")]
mod login_item;
mod palette;
pub mod settings;
mod settings_view;
pub mod shell;
pub mod theme;
#[cfg(feature = "tray")]
mod tray;
mod ui;

use gpui_kit::{
    Action, App, AppContext, Bounds, KeyBinding, Menu, MenuItem, OsAction, Pixels, Size,
    TitlebarOptions, WindowBounds, WindowOptions,
    component::{
        TitleBar,
        input::{Copy, Cut, Paste, Redo, SelectAll, Undo},
    },
    px, size,
};

pub use launch::{Launch, Page};
use settings::SettingsFile;
use shell::Shell;

pub const APP_NAME: &str = "{{app_name}}";

/// Also the size `just shot` renders at.
pub const MAIN_WINDOW_SIZE: Size<Pixels> = Size {
    width: px(720.0),
    height: px(480.0),
};

gpui_kit::actions!(
    {{crate_name}},
    [
        Quit,
        About,
        NewItem,
        ShowHome,
        OpenSettings,
        CloseWindow,
        TogglePalette
    ]
);

#[derive(Clone, Debug, PartialEq, gpui_kit::Action)]
#[action(namespace = {{crate_name}}, no_json)]
pub struct SetAppearance(pub theme::Appearance);

/// `secondary` is ⌘ on macOS and Ctrl elsewhere. Only `KeyBinding::load` resolves it:
/// `KeyBinding::new` panics on `secondary-` at launch.
pub fn shortcuts() -> Vec<(&'static str, Box<dyn Action>)> {
    vec![
        ("secondary-q", Box::new(Quit)),
        ("secondary-n", Box::new(NewItem)),
        ("secondary-w", Box::new(CloseWindow)),
        ("secondary-1", Box::new(ShowHome)),
        ("secondary-,", Box::new(OpenSettings)),
        ("secondary-k", Box::new(TogglePalette)),
    ]
}

/// Call once, before opening any window.
pub fn init(cx: &mut App) {
    gpui_kit::init(cx);
    theme::install(cx);
    let bindings: Vec<KeyBinding> = shortcuts()
        .into_iter()
        .map(|(keys, action)| {
            KeyBinding::load(keys, action, None, false, None, cx)
                .unwrap_or_else(|err| panic!("invalid shortcut {keys:?}: {err}"))
        })
        .collect();
    cx.bind_keys(bindings);
}

/// Returns after the app quits.
pub fn run(launch: Launch) {
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);
    // A tray app has no Dock icon. Set inside `run`, the policy comes too late and the icon
    // flashes. On macOS `Headless` means exactly `ActivationPolicy::Accessory`; on Linux it
    // would mean no display server.
    #[cfg(all(feature = "tray", target_os = "macos"))]
    let app = app.with_windowing(gpui_kit::WindowingRequest::Headless);
    app.on_reopen(show_main_window);
    app.run(move |cx: &mut App| {
        init(cx);
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());
        cx.set_menus(menus());

        let dry_run = launch.dry_run;
        cx.set_global(launch);

        #[cfg(feature = "tray")]
        tray::install(cx);

        if let Err(err) = open_main_window(cx) {
            eprintln!("{APP_NAME}: could not open the main window: {err}");
            std::process::exit(1);
        }
        if !dry_run {
            cx.activate(true);
        }
    });
}

/// Brings the main window forward, reopening it if it was closed. The tray calls this
/// directly: with no window focused, a dispatched action reaches nothing.
pub fn show_main_window(cx: &mut App) {
    if let Some(window) = cx.windows().into_iter().next() {
        let activated = window.update(cx, |_, window, _| window.activate_window());
        if activated.is_ok() {
            return;
        }
    }
    if let Err(err) = open_main_window(cx) {
        eprintln!("{APP_NAME}: could not open the main window: {err}");
    }
}

fn open_main_window(cx: &mut App) -> gpui_kit::Result<()> {
    let launch = cx.global::<Launch>().clone();
    let file = SettingsFile::for_app(launch.dry_run);
    let options = main_window_options(&launch, cx);
    gpui_kit::open_window(options, cx, move |window, cx| {
        cx.new(|cx| Shell::new(&launch, file, window, cx))
    })?;
    Ok(())
}

fn main_window_options(launch: &Launch, cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            MAIN_WINDOW_SIZE,
            cx,
        ))),
        window_min_size: Some(size(px(560.0), px(400.0))),
        focus: !launch.dry_run,
        // Matches the `.desktop` file `just icon-linux` writes (Wayland).
        app_id: Some(env!("CARGO_PKG_NAME").into()),
        // Not drawn, but names the window in the Window menu and Mission Control.
        titlebar: Some(TitlebarOptions {
            title: Some(APP_NAME.into()),
            ..TitleBar::title_bar_options()
        }),
        ..TitleBar::window_options()
    }
}

fn menus() -> Vec<Menu> {
    vec![
        Menu {
            name: APP_NAME.into(),
            disabled: false,
            items: vec![
                MenuItem::action(format!("About {APP_NAME}"), About),
                MenuItem::separator(),
                MenuItem::action("Settings…", OpenSettings),
                MenuItem::separator(),
                MenuItem::action(format!("Quit {APP_NAME}"), Quit),
            ],
        },
        Menu {
            name: "File".into(),
            disabled: false,
            items: vec![
                MenuItem::action("New Item…", NewItem),
                MenuItem::separator(),
                MenuItem::action("Close Window", CloseWindow),
            ],
        },
        Menu {
            name: "Edit".into(),
            disabled: false,
            items: vec![
                MenuItem::os_action("Undo", Undo, OsAction::Undo),
                MenuItem::os_action("Redo", Redo, OsAction::Redo),
                MenuItem::separator(),
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
            ],
        },
        Menu {
            name: "View".into(),
            disabled: false,
            items: vec![
                MenuItem::action("Command Palette…", TogglePalette),
                MenuItem::separator(),
                MenuItem::action("Home", ShowHome),
                MenuItem::action("Settings", OpenSettings),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Keystroke, OperatingSystem};

    #[test]
    fn every_shortcut_parses_on_macos_and_linux() {
        for os in [OperatingSystem::Mac, OperatingSystem::Linux] {
            for (keys, action) in super::shortcuts() {
                for keystroke in keys.split_whitespace() {
                    if let Err(err) = Keystroke::parse_with_os(keystroke, os) {
                        panic!(
                            "{keys:?} ({}) does not parse on {os:?}: {err}",
                            action.name()
                        );
                    }
                }
            }
        }
    }
}
