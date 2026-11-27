//! The ⌘K command palette. It renders inside `Shell`, not in a `Dialog`: rows dispatch the
//! app's actions from the palette's focus, and a dialog sits outside Shell's element tree,
//! where those actions would never reach Shell's handlers.

use gpui_kit::{
    AnyElement, Context, Entity, FocusHandle, Focusable as _, MouseButton, Window,
    component::{
        ActiveTheme as _, FocusTrapElement as _, IconName,
        command::{Command, CommandGroup, CommandItem, CommandState},
    },
    div,
    prelude::*,
    px,
};

use crate::{
    APP_NAME, NewItem, OpenSettings, Quit, SetAppearance, ShowHome, TogglePalette, shell::Shell,
    theme::Appearance,
};

pub(crate) struct Palette {
    state: Entity<CommandState>,
    container: FocusHandle,
}

impl Shell {
    pub(crate) fn toggle_palette(
        &mut self,
        _: &TogglePalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.palette.is_some() {
            self.close_palette(window, cx);
            return;
        }
        let state = cx.new(|cx| CommandState::new(window, cx));
        state.update(cx, |state, cx| state.focus(window, cx));
        self.palette = Some(Palette {
            state,
            container: cx.focus_handle(),
        });
        cx.notify();
    }

    pub(crate) fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(palette) = self.palette.take() else {
            return;
        };
        if palette.container.contains_focused(window, cx) {
            window.focus(&self.focus_handle(cx), cx);
        }
        cx.notify();
    }

    pub(crate) fn render_palette(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let current = self.appearance();
        let confirmed = cx.entity().downgrade();
        let cancelled = confirmed.clone();

        let command = Command::new(&palette.state)
            .placeholder("Search commands…")
            .highlight_matches(true)
            // Fits every row at the default window size.
            .max_h(px(320.0))
            .group(
                CommandGroup::new()
                    .label("Go")
                    .item(row("Home", IconName::Inbox).action(Box::new(ShowHome)))
                    .item(
                        row("Settings", IconName::Settings)
                            .keywords(["preferences"])
                            .action(Box::new(OpenSettings)),
                    ),
            )
            .group(
                CommandGroup::new().label("Items").item(
                    row("New Item…", IconName::Plus)
                        .keywords(["add", "create"])
                        .action(Box::new(NewItem)),
                ),
            )
            .group(
                CommandGroup::new()
                    .label("Appearance")
                    .items(Appearance::ALL.map(|appearance| {
                        let icon = match appearance {
                            Appearance::System => IconName::Palette,
                            Appearance::Light => IconName::Sun,
                            Appearance::Dark => IconName::Moon,
                        };
                        row(appearance.label(), icon)
                            .keywords(["appearance", "theme", "mode"])
                            .checked(appearance == current)
                            .action(Box::new(SetAppearance(appearance)))
                    })),
            )
            .group(
                CommandGroup::new()
                    .label(APP_NAME)
                    .item(row("Quit", IconName::Close).action(Box::new(Quit))),
            )
            .on_confirm(move |_, window, cx| {
                drop(confirmed.update(cx, |this, cx| this.close_palette(window, cx)));
            })
            // Runs while the palette state is still being updated: close on the next turn.
            .on_cancel(move |window, cx| {
                let cancelled = cancelled.clone();
                window.defer(cx, move |window, cx| {
                    drop(cancelled.update(cx, |this, cx| this.close_palette(window, cx)));
                });
            });

        div()
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(48.0))
            .px_4()
            .bg(cx.theme().overlay)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.close_palette(window, cx)),
            )
            .child(
                div()
                    .w(px(480.0))
                    .max_w_full()
                    .rounded(cx.theme().radius_lg)
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(command)
                    .focus_trap("palette", &palette.container),
            )
            .into_any_element()
    }
}

fn row(label: &'static str, icon: IconName) -> CommandItem {
    CommandItem::new().label(label).icon(icon)
}
