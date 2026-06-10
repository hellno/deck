//! Bottom-center recording pill (`--features overlay`). Anchored bottom-center.
//!
//! A small frosted pill with a circular record button. `recording` flips on click
//! (and via the `space` ToggleRecording action wired in `mod.rs`); while recording,
//! the button shows a pulsing red fill. The window is transparent — paint only the pill.

use gpui::{
    div, px, Animation, AnimationExt, App, Context, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{h_flex, ActiveTheme};
use std::time::Duration;

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
        // Copy colors out so the `&Theme` borrow ends before the record button's
        // `cx.listener` re-borrows cx as `&mut` (Hsla is Copy). Mirrors hud.rs.
        let theme = cx.theme();
        let border = theme.border;
        let surface = theme.popover;
        let danger = theme.danger;
        let muted = theme.muted_foreground;

        // Button base tints: muted at rest, slightly stronger on hover/press. The red
        // signal lives in the inner dot, not the button fill.
        let rest = muted.opacity(0.25);
        let hover = muted.opacity(0.4);
        let press = muted.opacity(0.55);

        // The inner record dot. While recording it pulses red (opacity breathes between
        // 0.4 and 1.0); at rest it's a static muted circle. The two arms are different
        // element types (animated vs plain `div`), so unify via `into_any_element`.
        // CRITICAL: `with_animation` returns an opaque animated element — it is the dot
        // (a CHILD), never the clickable surface. The `.id("pill-record")` button below
        // is the stateful parent that carries `.on_click`.
        let dot = if self.recording {
            div()
                .size(px(12.0))
                .rounded_full()
                .bg(danger)
                .with_animation(
                    "pill-record-pulse",
                    Animation::new(Duration::from_millis(900))
                        .repeat()
                        .with_easing(gpui::pulsating_between(0.4, 1.0)),
                    |el, delta| el.opacity(delta),
                )
                .into_any_element()
        } else {
            div()
                .size(px(12.0))
                .rounded_full()
                .bg(muted)
                .into_any_element()
        };

        // Transparent full-window wrapper; the pill sits centered.
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                h_flex()
                    .id("overlay-pill")
                    .gap_2()
                    .items_center()
                    .px_3()
                    .py_2()
                    .rounded_full()
                    .border_1()
                    .border_color(border)
                    .bg(surface.opacity(0.85))
                    .shadow_lg()
                    // Record control — also the P2 focus-spike target: clicking over
                    // another app must NOT steal its key focus; we log the verdict.
                    .child(
                        div()
                            .id("pill-record")
                            .size(px(28.0))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rest)
                            .hover(move |s| s.bg(hover))
                            .active(move |s| s.bg(press))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                crate::overlay::harden::log_focus_state(window);
                                this.toggle(cx);
                            }))
                            .child(dot),
                    ),
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
