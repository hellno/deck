//! macOS panel hardening (P2): stop the overlay panel stealing key-window focus on
//! click, optionally drop the panel's OS window shadow (so a `rounded_full` pill isn't
//! framed by the window's rounded-rectangle shadow), plus T0 spike instrumentation.
//! See `docs/overlay.md` CHILD #4.
//!
//! The bridge recovers the live NSView from gpui's `HasWindowHandle` raw handle,
//! walks to its NSWindow, downcasts to NSPanel, and flips `becomesKeyOnlyIfNeeded`.
//! Everything but the raw-pointer `Retained::retain` recovery uses safe objc2-app-kit
//! wrappers; the single unsafe is fenced with `// SAFETY:` + scoped `#[allow]`.

#[cfg(target_os = "macos")]
pub fn harden_panel(window: &gpui::Window, hide_shadow: bool) {
    harden_panel_macos(window, hide_shadow);
}

#[cfg(not(target_os = "macos"))]
pub fn harden_panel(_window: &gpui::Window, _hide_shadow: bool) {}

#[cfg(target_os = "macos")]
fn harden_panel_macos(window: &gpui::Window, hide_shadow: bool) {
    use objc2::rc::Retained;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSPanel, NSView};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if MainThreadMarker::new().is_none() {
        return;
    }
    // GOTCHA: `Window` has an inherent `window_handle() -> AnyWindowHandle` that
    // shadows the rwh trait method. We need the trait one (the raw NSView), so call
    // it through `HasWindowHandle` explicitly.
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    let ns_view_ptr = appkit.ns_view.as_ptr().cast::<NSView>();
    // SAFETY: `appkit.ns_view` is gpui's live MacWindow native NSView (gpui_macos
    // HasWindowHandle), valid for this window's lifetime, which outlives this
    // synchronous main-thread call. `retain` takes a +1 reference without stealing
    // gpui's ownership; the Retained drops at scope end. We store no raw pointer.
    // NSView/NSWindow are MainThreadOnly; the marker check above guarantees we are on
    // the main thread.
    #[allow(unsafe_code)]
    let Some(view) = (unsafe { Retained::retain(ns_view_ptr) }) else {
        return;
    };
    let Some(ns_window) = view.window() else {
        return;
    };
    let Ok(panel) = ns_window.downcast::<NSPanel>() else {
        return;
    };
    panel.setBecomesKeyOnlyIfNeeded(true);
    if hide_shadow {
        // Drop the macOS window shadow for this panel. gpui gives every transparent
        // window a near-opaque backing so the OS still draws a shadow (gpui_macos
        // window.rs: "avoid broken shadow"), but that shadow is a rounded *rectangle*
        // the size of the window — it shows as a mismatched frame behind a `rounded_full`
        // pill. Killing it lets the panel's own rendered `shadow_lg` be the only depth cue.
        // Both the pill and the rail pass `true` — each is a content-sized panel centered in
        // a larger transparent canvas, so the window-sized shadow would just be a mismatched
        // frame around the empty canvas.
        panel.setHasShadow(false);
    }
}

/// Spike instrumentation (T0): log the panel's key state + the frontmost app so the
/// PASS/FAIL verdict is objective, not eyeballed. Call from a HUD button's on_click.
#[cfg(target_os = "macos")]
pub fn log_focus_state(window: &gpui::Window) {
    use objc2::rc::Retained;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSPanel, NSView, NSWorkspace};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if MainThreadMarker::new().is_none() {
        return;
    }
    // Use the rwh trait method explicitly (see `harden_panel_macos`).
    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    let ns_view_ptr = appkit.ns_view.as_ptr().cast::<NSView>();
    // SAFETY: same invariant as harden_panel_macos above.
    #[allow(unsafe_code)]
    let Some(view) = (unsafe { Retained::retain(ns_view_ptr) }) else {
        return;
    };
    let Some(ns_window) = view.window() else {
        return;
    };
    if let Ok(panel) = ns_window.downcast::<NSPanel>() {
        eprintln!("overlay spike: panel isKeyWindow = {}", panel.isKeyWindow());
    }
    let ws = NSWorkspace::sharedWorkspace();
    if let Some(app) = ws.frontmostApplication() {
        eprintln!("overlay spike: frontmost app = {:?}", app.localizedName());
    }
}

#[cfg(not(target_os = "macos"))]
pub fn log_focus_state(_window: &gpui::Window) {}
