//! The main window's root view: page state, settings and the title bar.

use gpui_kit::{
    App, Context, Entity, FocusHandle, Focusable, MouseButton, PromptLevel, SharedString,
    Subscription, Window,
    component::{
        ActiveTheme as _, IndexPath, Sizable as _, TitleBar,
        select::{SelectEvent, SelectState},
        tab::{Tab, TabBar},
        v_flex,
    },
    div,
    prelude::*,
    px,
};

use crate::{
    APP_NAME, About, CloseWindow, Launch, NewItem, OpenSettings, Page, SetAppearance, ShowHome,
    home,
    palette::Palette,
    settings::{Settings, SettingsFile},
    theme::{self, Appearance},
};

pub struct Shell {
    focus_handle: FocusHandle,
    pub(crate) page: Page,
    pub(crate) settings: Settings,
    pub(crate) file: SettingsFile,
    /// A one-off appearance for this run (`APP_APPEARANCE`); not saved.
    appearance_override: Option<Appearance>,
    pub(crate) settings_problem: Option<SharedString>,
    pub(crate) dry_run: bool,
    pub(crate) items: Vec<SharedString>,
    #[cfg(target_os = "macos")]
    pub(crate) login_item: crate::login_item::LoginItem,
    pub(crate) appearance_select: Entity<SelectState<Vec<&'static str>>>,
    pub(crate) palette: Option<Palette>,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub fn new(
        launch: &Launch,
        mut file: SettingsFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);

        let loaded = file.load();
        let settings = loaded.settings;
        let appearance = launch.appearance.unwrap_or(settings.appearance);
        theme::apply(appearance, window, cx);

        let selected = Appearance::ALL.iter().position(|a| *a == appearance);
        let appearance_select = cx.new(|cx| {
            SelectState::new(
                Appearance::ALL.map(Appearance::label).to_vec(),
                selected.map(IndexPath::new),
                window,
                cx,
            )
        });

        let subscriptions = vec![
            cx.subscribe_in(
                &appearance_select,
                window,
                |this, _, event: &SelectEvent<Vec<&'static str>>, window, cx| {
                    let SelectEvent::Confirm(Some(label)) = event else {
                        return;
                    };
                    if let Some(appearance) = Appearance::parse(label) {
                        this.set_appearance(appearance, window, cx);
                    }
                },
            ),
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.appearance() == Appearance::System {
                    theme::apply(Appearance::System, window, cx);
                }
            }),
        ];

        Self {
            focus_handle,
            page: launch.page,
            settings,
            file,
            appearance_override: launch.appearance,
            settings_problem: loaded.problem.map(Into::into),
            dry_run: launch.dry_run,
            items: if launch.fixtures {
                home::sample_items()
            } else {
                Vec::new()
            },
            #[cfg(target_os = "macos")]
            login_item: crate::login_item::LoginItem::detect(launch.dry_run),
            appearance_select,
            palette: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn appearance(&self) -> Appearance {
        self.appearance_override.unwrap_or(self.settings.appearance)
    }

    pub fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    pub(crate) fn set_appearance(
        &mut self,
        appearance: Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.appearance_override = None;
        self.settings.appearance = appearance;
        self.file.save_best_effort(&self.settings);
        theme::apply(appearance, window, cx);
        self.appearance_select.update(cx, |select, cx| {
            select.set_selected_value(&appearance.label(), window, cx);
        });
        cx.notify();
    }

    fn on_about(&mut self, _: &About, window: &mut Window, cx: &mut Context<Self>) {
        drop(window.prompt(
            PromptLevel::Info,
            APP_NAME,
            Some(concat!("Version ", env!("CARGO_PKG_VERSION"))),
            &["OK"],
            cx,
        ));
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = Page::ALL.iter().position(|p| *p == self.page).unwrap_or(0);
        let tabs = TabBar::new("pages")
            .segmented()
            .small()
            .selected_index(selected)
            .on_click(cx.listener(|this, index: &usize, _, cx| {
                if let Some(page) = Page::ALL.get(*index) {
                    this.navigate(*page, cx);
                }
            }))
            .children(Page::ALL.map(|page| Tab::new().label(page.label())));

        TitleBar::new().child(
            div()
                .relative()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                // Centre on the window, not on the space right of the traffic lights.
                .when(cfg!(target_os = "macos"), |this| this.mr(px(80.0)))
                .child(
                    // The title bar drags the window on mouse down unless a child prevents
                    // it, and the tabs don't. Without this, clicking a tab moves the window.
                    div()
                        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                        .child(tabs),
                )
                .when(self.dry_run, |this| {
                    this.child(
                        div()
                            .absolute()
                            .right_3()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Dry run"),
                    )
                }),
        )
    }
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = match self.page {
            Page::Home => self.render_home(cx).into_any_element(),
            Page::Settings => self.render_settings(cx).into_any_element(),
        };
        let palette = self
            .palette
            .as_ref()
            .map(|palette| self.render_palette(palette, cx));
        v_flex()
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &NewItem, window, cx| this.new_item(window, cx)))
            .on_action(cx.listener(|this, _: &ShowHome, _, cx| this.navigate(Page::Home, cx)))
            .on_action(
                cx.listener(|this, _: &OpenSettings, _, cx| this.navigate(Page::Settings, cx)),
            )
            .on_action(cx.listener(|_, _: &CloseWindow, window, _| window.remove_window()))
            .on_action(cx.listener(Self::on_about))
            .on_action(cx.listener(
                |this, SetAppearance(appearance): &SetAppearance, window, cx| {
                    this.set_appearance(*appearance, window, cx);
                },
            ))
            .on_action(cx.listener(Self::toggle_palette))
            .child(self.render_title_bar(cx))
            .child(page)
            .children(palette)
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{
        AnyWindowHandle, App, AppContext as _, Entity, SharedString, TestAppContext, Window,
        test::TestWindowExt as _,
    };

    use super::Shell;
    use crate::{Launch, Page, settings::SettingsFile};

    const PALETTE: &str = if cfg!(target_os = "macos") {
        "cmd-k"
    } else {
        "ctrl-k"
    };
    const NEW_ITEM: &str = if cfg!(target_os = "macos") {
        "cmd-n"
    } else {
        "ctrl-n"
    };

    fn open(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Shell>) {
        cx.update(crate::init);
        cx.update(|cx| {
            gpui_kit::open_window(Default::default(), cx, |window, cx| {
                cx.new(|cx| Shell::new(&Launch::default(), SettingsFile::none(), window, cx))
            })
        })
        .expect("open a window")
    }

    fn step(
        cx: &mut TestAppContext,
        window: AnyWindowHandle,
        act: impl FnOnce(&mut Window, &mut App),
    ) {
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            act(window, cx);
        })
        .expect("window is open");
        cx.run_until_parked();
    }

    #[gpui_kit::test]
    fn a_palette_command_runs_and_closes_the_palette(cx: &mut TestAppContext) {
        let (window, shell) = open(cx);
        step(cx, window, |window, cx| window.press(PALETTE, cx));
        step(cx, window, |window, cx| window.input("settings", cx));
        step(cx, window, |window, cx| window.press("enter", cx));
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.page, Page::Settings);
            assert!(shell.palette.is_none());
        });
    }

    #[gpui_kit::test]
    fn the_palette_keeps_focus_until_escape(cx: &mut TestAppContext) {
        let (window, shell) = open(cx);
        step(cx, window, |window, cx| window.press(PALETTE, cx));
        step(cx, window, |window, cx| window.press("tab", cx));
        step(cx, window, |window, cx| window.press("escape", cx));
        shell.read_with(cx, |shell, _| assert!(shell.palette.is_none()));
    }

    #[gpui_kit::test]
    fn new_item_takes_typing_right_away(cx: &mut TestAppContext) {
        let (window, shell) = open(cx);
        step(cx, window, |window, cx| window.press(NEW_ITEM, cx));
        step(cx, window, |window, cx| window.input("Milk", cx));
        step(cx, window, |window, cx| window.press("enter", cx));
        shell.read_with(cx, |shell, _| {
            assert_eq!(shell.items, [SharedString::from("Milk")]);
        });
    }
}
