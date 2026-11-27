use gpui_kit::{
    Context,
    component::{ActiveTheme as _, alert::Alert, select::Select, separator::Separator, v_flex},
    div,
    prelude::*,
    px,
};

use crate::{shell::Shell, ui};

impl Shell {
    pub(crate) fn render_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let appearance = ui::row(
            "Appearance",
            Some("Light, dark, or match the system".into()),
            div()
                .flex_none()
                .w(px(140.0))
                .child(Select::new(&self.appearance_select).accessibility_label("Appearance")),
            cx,
        );
        let saved_to = match self.file.path() {
            Some(path) => format!("Settings are saved to {}", path.display()),
            None => "Settings aren't saved in this session.".to_string(),
        };

        v_flex().flex_1().items_center().p_6().child(
            v_flex()
                .w(px(480.0))
                .gap_4()
                .children(
                    self.settings_problem
                        .clone()
                        .map(|problem| Alert::warning("settings-problem", problem)),
                )
                .child(
                    ui::group("General").child(appearance).children(
                        self.render_login_item(cx)
                            .map(|row| v_flex().child(Separator::horizontal()).child(row)),
                    ),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(saved_to),
                ),
        )
    }

    #[cfg(target_os = "macos")]
    fn render_login_item(&self, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        use gpui_kit::component::{Disableable as _, switch::Switch};

        let detail = if self.login_item.available() {
            "Start automatically when you log in"
        } else if self.dry_run {
            "Off during a dry run"
        } else {
            "Available when the app is in Applications"
        };
        let switch = Switch::new("open-at-login")
            .checked(self.login_item.enabled())
            .disabled(!self.login_item.available())
            .accessibility_label("Open at login")
            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                if let Err(err) = this.login_item.set_enabled(*checked) {
                    eprintln!(
                        "{}: could not change the login item: {err}",
                        crate::APP_NAME
                    );
                }
                cx.notify();
            }));
        Some(ui::row("Open at login", Some(detail.into()), switch, cx).into_any_element())
    }

    #[cfg(not(target_os = "macos"))]
    fn render_login_item(&self, _: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        None
    }
}
