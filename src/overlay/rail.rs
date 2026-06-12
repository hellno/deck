//! Top-right job-status + action-button rail (`--features overlay`). Anchored top-right.
//!
//! A small frosted vertical panel with three stacked sections: one status icon per
//! background job (pulsing while `Running`, settling on `Done`/`Failed`), a thin
//! divider, then three square action buttons. The job icons are the generic
//! background-job proof (see `status.rs`); the buttons demonstrate clickable hover
//! controls (a `pinned` toggle that stays visibly filled, plus two momentary
//! buttons). The window is transparent — paint only the frosted panel.

use crate::overlay::status::JobStatus;
use gpui::{
    div, px, Animation, AnimationExt, App, Context, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{v_flex, ActiveTheme, Icon, IconName};
use std::time::Duration;

pub struct Rail {
    pub focus_handle: FocusHandle,
    pub jobs: Vec<JobStatus>,
    /// Toggle button #1 ("pin"-like). Stays visibly filled while true.
    pub pinned: bool,
}

impl Rail {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            // Seed three idle jobs so the rail shows status icons from the very first
            // frame; the demo spine in `mod.rs` then advances them (Idle -> Running ->
            // Done/Failed). Without this the rail is iconless until the first 2s tick.
            jobs: vec![JobStatus::Idle; 3],
            pinned: false,
        }
    }
}

impl Focusable for Rail {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Rail {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Copy colors out so the `&Theme` borrow ends before the buttons' `cx.listener`
        // closures re-borrow cx as `&mut` (Hsla is Copy). Mirrors pill.rs / status_row.rs.
        let theme = cx.theme();
        let border = theme.border;
        let surface = theme.popover;
        let muted = theme.muted_foreground;
        let primary = theme.primary;
        let success = theme.success;
        let danger = theme.danger;

        // Per-button tints derived from the locals above. Neutral rest fill, slightly
        // stronger on hover/press; the pinned toggle gets a primary tint while active.
        let neutral_rest = muted.opacity(0.15);
        let neutral_hover = muted.opacity(0.3);
        let neutral_press = muted.opacity(0.45);
        let pinned_rest = primary.opacity(0.25);
        let pinned_hover = primary.opacity(0.35);
        let pinned_press = primary.opacity(0.45);

        // SECTION 1 — one status icon per job. A `Running` job's icon pulses via
        // `with_animation` + `pulsating_between`; settled states render static in a
        // distinct theme color. Running vs static are different element types, so each
        // is pushed into `jobs_col` individually (the animated arm is an opaque element;
        // never attach `.id()`/`.on_click()` after `with_animation`).
        let mut jobs_col = v_flex().items_center().gap_2();
        for (idx, job) in self.jobs.iter().enumerate() {
            let (name, color) = match job {
                JobStatus::Idle => (IconName::Dash, muted),
                JobStatus::Running { .. } => (IconName::LoaderCircle, primary),
                JobStatus::Done(_) => (IconName::CircleCheck, success),
                JobStatus::Failed(_) => (IconName::CircleX, danger),
            };

            let icon = Icon::new(name).text_color(color);

            if job.is_running() {
                // Pulse the running icon's opacity to signal live work. A unique,
                // per-row id keeps each animation's element state independent.
                jobs_col = jobs_col.child(
                    icon.with_animation(
                        ("rail-job", idx),
                        Animation::new(Duration::from_secs(1))
                            .repeat()
                            .with_easing(gpui::pulsating_between(0.4, 1.0)),
                        |icon, delta| icon.opacity(delta),
                    ),
                );
            } else {
                jobs_col = jobs_col.child(icon);
            }
        }

        // Button 1 — toggle "pin": filled while active so the state is visible.
        let (pin_icon, pin_rest, pin_hover, pin_press) = if self.pinned {
            (IconName::StarFill, pinned_rest, pinned_hover, pinned_press)
        } else {
            (IconName::Star, neutral_rest, neutral_hover, neutral_press)
        };
        let pin_button = div()
            .id("rail-pin")
            .size(px(34.0))
            .rounded_lg()
            .flex()
            .items_center()
            .justify_center()
            .bg(pin_rest)
            .hover(move |s| s.bg(pin_hover))
            .active(move |s| s.bg(pin_press))
            .cursor_pointer()
            .on_click(cx.listener(|this, _ev, _window, cx| {
                this.pinned = !this.pinned;
                eprintln!("overlay rail: pin -> {}", this.pinned);
                cx.notify();
            }))
            .child(Icon::new(pin_icon).text_color(primary));

        // Button 2 — momentary "eye": one-line stderr marker on click.
        let eye_button = div()
            .id("rail-eye")
            .size(px(34.0))
            .rounded_lg()
            .flex()
            .items_center()
            .justify_center()
            .bg(neutral_rest)
            .hover(move |s| s.bg(neutral_hover))
            .active(move |s| s.bg(neutral_press))
            .cursor_pointer()
            .on_click(cx.listener(|_this, _ev, _window, _cx| {
                eprintln!("overlay rail: eye clicked");
            }))
            .child(Icon::new(IconName::Eye).text_color(muted));

        // Button 3 — momentary "bell": one-line stderr marker on click.
        let bell_button = div()
            .id("rail-bell")
            .size(px(34.0))
            .rounded_lg()
            .flex()
            .items_center()
            .justify_center()
            .bg(neutral_rest)
            .hover(move |s| s.bg(neutral_hover))
            .active(move |s| s.bg(neutral_press))
            .cursor_pointer()
            .on_click(cx.listener(|_this, _ev, _window, _cx| {
                eprintln!("overlay rail: bell clicked");
            }))
            .child(Icon::new(IconName::Bell).text_color(muted));

        // Transparent full-window wrapper; the content-sized frosted panel floats centered
        // via its OWN `shadow_lg`. The window's OS shadow is disabled in `mod.rs`
        // (`harden_panel(.., true)`), so the panel isn't framed by a mismatched rounded-rect
        // window shadow around the transparent canvas — matching the pill. Borderless for the
        // same clean, unframed look; everything outside the rounded panel shows through.
        div()
            .size_full()
            .flex()
            .items_start()
            .justify_center()
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .p_2()
                    .rounded_xl()
                    .bg(surface.opacity(0.9))
                    .shadow_lg()
                    .child(jobs_col)
                    // SECTION 2 — divider: a thin horizontal line separating jobs from actions.
                    .child(div().h(px(1.0)).w(px(40.0)).bg(border))
                    // SECTION 3 — the three square action buttons.
                    .child(
                        v_flex()
                            .gap_2()
                            .child(pin_button)
                            .child(eye_button)
                            .child(bell_button),
                    ),
            )
    }
}
