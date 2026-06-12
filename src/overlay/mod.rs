//! Floating overlay surfaces (`--features overlay`).
//!
//! Two small, genuinely-transparent, borderless, always-on-top windows
//! (`WindowKind::PopUp`): a top-right job-status + action-button **rail** and a
//! bottom-center recording **pill**. macOS-only in v1; Linux compiles to a no-op
//! (no LayerShell yet). See `docs/overlay.md` for the full design.
//!
//! Each window's `open_window` build closure returns the view entity DIRECTLY (no
//! gpui-component `Root` wrapper) so the window stays transparent — `Root` paints an
//! opaque theme background, which was the old "dark box". The simplified surfaces
//! need no tooltips/notifications/modals, so `Root` is unnecessary.
//!
//! Spine (the generic background-job pattern, `docs/background-jobs.md`): push state
//! into the rail from a background task via a `WeakEntity<rail::Rail>` stashed in a
//! GPUI `Global`, then `cx.notify()`. `upgrade()` returning `None` is the natural
//! no-op after the overlay closes — no panic, no leaked task.

mod harden;
mod pill;
mod rail;
mod state;
mod status;

#[cfg(target_os = "macos")]
use gpui::AppContext;
#[cfg(target_os = "macos")]
use state::OverlayAnchor;

// The recording toggle action. Declared cross-platform (acceptance criterion #8: the
// action + its keybinding must compile on Linux); the *handler* only does work on
// macOS, where the pill window exists.
gpui::actions!({{crate_name}}, [ToggleRecording]);

// Globals holding a weak handle to each live overlay view (for background pushes +
// the recording toggle) plus the window's root handle (held for lifecycle/teardown).
// macOS-only: Linux never opens the windows, so constructing these would be dead code.
#[cfg(target_os = "macos")]
struct RailHandle {
    rail: gpui::WeakEntity<rail::Rail>,
    // `#[allow(dead_code)]`: held for lifecycle/teardown — the `on_window_closed`
    // subscription owns its own `WindowHandle` copy and drives the actual teardown,
    // so this stashed handle isn't read back yet. Mirrors the old `OverlayHandle.window`.
    #[allow(dead_code)]
    window: gpui::WindowHandle<rail::Rail>,
}

#[cfg(target_os = "macos")]
impl gpui::Global for RailHandle {}

#[cfg(target_os = "macos")]
struct PillHandle {
    pill: gpui::WeakEntity<pill::RecordingPill>,
    // `#[allow(dead_code)]`: held for lifecycle/teardown (see `RailHandle.window`).
    #[allow(dead_code)]
    window: gpui::WindowHandle<pill::RecordingPill>,
}

#[cfg(target_os = "macos")]
impl gpui::Global for PillHandle {}

/// Install the overlay surfaces if enabled. Threads the MAIN window handle so closing
/// the main window tears the overlay down with it.
pub fn install(
    cx: &mut gpui::App,
    settings: &crate::settings::Settings,
    main_window: gpui::WindowHandle<gpui_component::Root>,
) {
    // Effective config = the persisted setting, overridable per-run by an env var so
    // you can flip the overlay without editing settings.json (handy while developing,
    // and a ready-made hook a fork can keep, extend, or delete):
    //   DECK_OVERLAY=1|0|true|false   force the overlay on/off
    let enabled = env_override_bool("DECK_OVERLAY").unwrap_or(settings.overlay_enabled);

    if !enabled {
        let _ = main_window;
        return;
    }

    // Recording toggle: click the pill's record button (primary), OR press `space`
    // while Deck is the focused app. The binding is context-scoped so it never eats a
    // space typed into a text field. Registered cross-platform (it must compile on
    // Linux); the handler only finds a pill on macOS, where one exists.
    cx.bind_keys([gpui::KeyBinding::new(
        "space",
        ToggleRecording,
        Some("!Input && !NumberInput && !SearchPanel"),
    )]);
    cx.on_action(|_: &ToggleRecording, cx: &mut gpui::App| {
        #[cfg(target_os = "macos")]
        if let Some(pill) = cx.try_global::<PillHandle>().and_then(|h| h.pill.upgrade()) {
            pill.update(cx, |p, cx| p.toggle(cx));
        }
        #[cfg(not(target_os = "macos"))]
        let _ = cx;
    });

    #[cfg(target_os = "macos")]
    install_macos(cx, main_window);
    #[cfg(not(target_os = "macos"))]
    {
        // Linux/Windows: compile-only no-op (no LayerShell in v1).
        let _ = (cx, main_window);
    }
}

/// Read a boolean env override, warning (and falling back to the setting) on an
/// unrecognized value so a typo isn't swallowed silently.
fn env_override_bool(key: &str) -> Option<bool> {
    let raw = std::env::var(key).ok()?;
    let parsed = parse_bool_override(&raw);
    if parsed.is_none() {
        eprintln!("{{project-name}}: ignoring {key}={raw:?} (expected 1/0/true/false)");
    }
    parsed
}

/// Pure parse of a truthy/falsy override string. Unknown → `None` (caller falls back).
fn parse_bool_override(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" => Some(true),
        "0" | "false" | "off" | "no" => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn install_macos(cx: &mut gpui::App, main_window: gpui::WindowHandle<gpui_component::Root>) {
    // Anchor both surfaces to the display the MAIN window is on (the active display),
    // falling back to the primary. `visible_bounds()` excludes the dock/menu bar — so a
    // BottomCenter pill clears the dock — and it carries the display's real origin
    // (macOS `bounds()` zeroes the origin, which would otherwise force every overlay
    // onto the primary display).
    let active_display = main_window
        .update(cx, |_, window, cx| window.display(cx))
        .ok()
        .flatten();
    let Some(display) = active_display.or_else(|| cx.primary_display()) else {
        return;
    };
    let vis = display.visible_bounds();

    // Windows sized to their content (not the old 460x160), shrinking the
    // transparent-pixel dead-zone around each surface.
    let rail_canvas = gpui::size(gpui::px(76.0), gpui::px(300.0));
    // The pill is content-sized and centered in this window with its OWN `shadow_lg`;
    // the window's OS shadow is disabled (see `harden_panel(.., true)`), so this window
    // is invisible except the pill. Leave margin around the pill for its rendered shadow.
    let pill_canvas = gpui::size(gpui::px(190.0), gpui::px(64.0));
    let rail_origin = OverlayAnchor::TopRight.origin(vis, rail_canvas, gpui::px(16.0));
    let pill_origin = OverlayAnchor::BottomCenter.origin(vis, pill_canvas, gpui::px(16.0));

    // Shared window flags: transparent, borderless, non-activating PopUp panels that
    // never resize/minimize and never appear in the dock or window list.
    let window_opts = |origin, size| gpui::WindowOptions {
        window_bounds: Some(gpui::WindowBounds::Windowed(gpui::Bounds { origin, size })),
        titlebar: None,
        focus: false,
        kind: gpui::WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display.id()),
        window_background: gpui::WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };

    // RAIL window — return the view entity DIRECTLY (no `Root` wrapper) so the window
    // stays transparent; the view paints only the frosted panel.
    let rail_opts = window_opts(rail_origin, rail_canvas);
    let Ok(rail_handle) = cx.open_window(rail_opts, |window, cx| {
        let rail = cx.new(|cx| rail::Rail::new(window, cx));
        let wh = window
            .window_handle()
            .downcast::<rail::Rail>()
            .expect("overlay rail window root is Rail");
        cx.set_global(RailHandle {
            rail: rail.downgrade(),
            window: wh,
        });
        // Drop the rail window's OS shadow (like the pill). The frosted panel is
        // content-sized and floats via its own `shadow_lg`, centered in the transparent
        // canvas — so the window's rounded-rect shadow would otherwise frame the whole
        // empty canvas (a mismatched border around the panel). harden.rs: `true` = no shadow.
        crate::overlay::harden::harden_panel(window, true);
        rail
    }) else {
        return;
    };

    // PILL window — same transparent, no-`Root` treatment.
    let pill_opts = window_opts(pill_origin, pill_canvas);
    let Ok(pill_handle) = cx.open_window(pill_opts, |window, cx| {
        let pill = cx.new(|cx| pill::RecordingPill::new(window, cx));
        let wh = window
            .window_handle()
            .downcast::<pill::RecordingPill>()
            .expect("overlay pill window root is RecordingPill");
        cx.set_global(PillHandle {
            pill: pill.downgrade(),
            window: wh,
        });
        // Drop the pill window's OS shadow — the pill is `rounded_full` and renders its
        // own `shadow_lg`, so the window's rounded-rect shadow would just be a mismatched
        // frame behind it.
        crate::overlay::harden::harden_panel(window, true);
        pill
    }) else {
        // Keep install transactional: the pill failed to open, so tear down the rail we
        // just opened (and its global). The redesign shows both surfaces together or
        // neither — never a lone rail.
        let _ = rail_handle.update(cx, |_, window, _| window.remove_window());
        if cx.has_global::<RailHandle>() {
            let _ = cx.remove_global::<RailHandle>();
        }
        return;
    };

    // Lifecycle. `on_window_closed` is a single global subscription that fires for ANY
    // window close with its `WindowId`, so we filter by id:
    //   - rail/pill window closed -> clear that global (later pushes upgrade() -> None);
    //   - main window closed      -> close both overlay windows too.
    // The overlay NEVER keeps the app alive and NEVER calls cx.quit()/cx.activate().
    let rail_id = rail_handle.window_id();
    let pill_id = pill_handle.window_id();
    let main_id = main_window.window_id();
    cx.on_window_closed(move |cx, closed_id| {
        if closed_id == main_id {
            // Close both overlay windows; ignore the Result (already-closed is fine).
            let _ = rail_handle.update(cx, |_, window, _| window.remove_window());
            let _ = pill_handle.update(cx, |_, window, _| window.remove_window());
        } else if closed_id == rail_id {
            if cx.has_global::<RailHandle>() {
                let _ = cx.remove_global::<RailHandle>();
            }
        } else if closed_id == pill_id && cx.has_global::<PillHandle>() {
            let _ = cx.remove_global::<PillHandle>();
        }
    })
    .detach();

    // Demo push task: proves the WeakEntity + notify spine off the UI thread. A
    // background timer hops back to the main thread, upgrades the rail's weak handle,
    // advances the generic job statuses, and notifies. The pill has NO demo — it
    // toggles on click/space only. Exits cleanly once the rail is gone (no leak).
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            let done = cx.update(|cx| {
                let Some(rail) = cx.try_global::<RailHandle>().and_then(|h| h.rail.upgrade())
                else {
                    return true;
                };
                rail.update(cx, |r, cx| {
                    crate::overlay::status::demo_advance_jobs(&mut r.jobs);
                    cx.notify();
                });
                false
            });
            if done {
                // Rail gone -> task exits (no leak).
                break;
            }
        }
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::parse_bool_override;

    #[test]
    fn parse_bool_override_accepts_common_spellings_and_rejects_junk() {
        for s in ["1", "true", "TRUE", " on ", "yes"] {
            assert_eq!(parse_bool_override(s), Some(true), "{s:?}");
        }
        for s in ["0", "false", "Off", "no"] {
            assert_eq!(parse_bool_override(s), Some(false), "{s:?}");
        }
        assert_eq!(parse_bool_override("maybe"), None);
        assert_eq!(parse_bool_override(""), None);
    }
}
