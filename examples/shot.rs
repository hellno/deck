//! `just shot <page> <light|dark> <out.png>`: renders the root view to a 2x PNG with the
//! headless GPU renderer, with no window. Other `APP_*` variables apply as in the app.

use std::{path::Path, process::ExitCode, sync::Arc, time::Duration};

use gpui_kit::{
    AppContext as _, Bounds, HeadlessAppContext, WindowBounds, WindowOptions,
    test::TestWindowExt as _,
};

use {{crate_name}}::settings::SettingsFile;
use {{crate_name}}::shell::Shell;
use {{crate_name}}::theme::Appearance;
use {{crate_name}}::{Launch, Page};

const USAGE: &str = "usage: just shot <home|settings> <light|dark> <out.png>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [page, appearance, out] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let (Some(page), Some(appearance @ (Appearance::Light | Appearance::Dark))) =
        (Page::parse(page), Appearance::parse(appearance))
    else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let launch = match Launch::from_env() {
        Ok(launch) => Launch {
            page,
            appearance: Some(appearance),
            ..launch
        },
        Err(err) => {
            eprintln!("shot: {err}");
            return ExitCode::from(2);
        }
    };
    match shoot(&launch, Path::new(out)) {
        Ok(()) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("shot: {err:#}");
            ExitCode::FAILURE
        }
    }
}

/// Must run on the main thread (the macOS platform requires it).
fn shoot(launch: &Launch, out: &Path) -> gpui_kit::Result<()> {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update({{crate_name}}::init);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Default::default(),
            size: {{crate_name}}::MAIN_WINDOW_SIZE,
        })),
        focus: false,
        show: false,
        ..Default::default()
    };
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Shell::new(launch, SettingsFile::none(), window, cx))
        })
    })?;
    // Draw once, then let entrance animations finish on the test clock before the frame
    // that gets captured; otherwise anything animating is caught at its first frame.
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    cx.advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    cx.capture_screenshot(handle)?.save(out)?;
    Ok(())
}
