//! Bottom-center recording pill (`--features overlay`). Anchored bottom-center.
//!
//! A single frosted, borderless pill — content-sized and centered, floating via its own
//! `shadow_lg`. The window's OS shadow is disabled (`mod.rs` → `harden_panel(.., true)`)
//! so the window is invisible and the `rounded_full` pill isn't framed by a mismatched
//! rounded-rectangle window shadow. Inside: a subtle light circular button + an inline
//! label. Idle shows an empty circle next to "Start"; while `recording` a solid red dot
//! fills the circle next to a bold "Recording" (no animation — a steady, calm "on"
//! state). The WHOLE pill is the click target; `recording` also flips via the `space`
//! ToggleRecording action wired in `mod.rs`.

use gpui::{
    div, px, App, Context, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Render, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{h_flex, ActiveTheme};

pub struct RecordingPill {
    pub focus_handle: FocusHandle,
    pub recording: bool,
}

impl RecordingPill {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            recording: false,
        }
    }

    /// Pure reducer (unit-tested). Flips the recording flag.
    pub fn toggled(recording: bool) -> bool {
        !recording
    }

    /// Apply the toggle to self + notify. Called from on_click AND the ToggleRecording action.
    pub fn toggle(&mut self, cx: &mut Context<Self>) {
        self.recording = Self::toggled(self.recording);
        cx.notify();
    }
}

impl Focusable for RecordingPill {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for RecordingPill {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Copy colors out so the `&Theme` borrow ends before the pill's `cx.listener`
        // re-borrows cx as `&mut` (Hsla is Copy). Mirrors rail.rs / the old hud.rs.
        let theme = cx.theme();
        let surface = theme.popover;
        let foreground = theme.foreground;
        let muted = theme.muted_foreground;
        let danger = theme.danger;

        // Subtle, light circular button (theme-aware faint tint — never the old heavy
        // dark disc). It holds a solid red dot ONLY while recording; idle it's empty.
        let mut circle = div()
            .size(px(20.0))
            .rounded_full()
            .bg(foreground.opacity(0.1))
            .flex()
            .items_center()
            .justify_center();
        if self.recording {
            // Solid red dot — steady, no animation (a calm, clear "on" state).
            circle = circle.child(div().size(px(10.0)).rounded_full().bg(danger));
        }

        // Inline label on the same baseline as the circle: bold "Recording" while active,
        // muted "Start" at rest.
        let (label_text, label_color) = if self.recording {
            ("Recording", foreground)
        } else {
            ("Start", muted)
        };
        let mut label = div().text_sm().text_color(label_color);
        if self.recording {
            label = label.font_weight(FontWeight::BOLD);
        }
        let label = label.child(label_text);

        // Transparent full-window wrapper; the content-sized frosted pill sits centered.
        // The pill renders its OWN `shadow_lg` for depth, and the window's OS shadow is
        // disabled in `mod.rs` (`harden_panel(.., true)`) — so the window is invisible
        // and there's no mismatched rounded-rect frame behind the `rounded_full` pill,
        // just the pill floating. The pill is the click + P2 focus-spike target: clicking
        // over another app must NOT steal its key focus — we log the verdict.
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                h_flex()
                    .id("overlay-pill")
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .px_3()
                    .py_1p5()
                    .rounded_full()
                    .bg(surface.opacity(0.9))
                    .shadow_lg()
                    .cursor_pointer()
                    .hover(move |s| s.bg(surface.opacity(0.95)))
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        crate::overlay::harden::log_focus_state(window);
                        this.toggle(cx);
                    }))
                    .child(circle)
                    .child(label),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggled_flips_recording() {
        assert!(RecordingPill::toggled(false));
        assert!(!RecordingPill::toggled(true));
        // Toggling twice returns to the original `false`.
        assert!(!RecordingPill::toggled(RecordingPill::toggled(false)));
    }
}
