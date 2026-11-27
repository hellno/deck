use gpui_kit::{
    Context, SharedString, Window,
    component::{
        Icon, IconName, WindowExt as _,
        button::{Button, ButtonVariants as _},
        dialog::{DialogAction, DialogClose, DialogFooter},
        empty::{Empty, EmptyContent, EmptyHeader, EmptyMedia, EmptyTitle},
        form::{field, v_form},
        h_flex,
        input::{Input, InputState},
        separator::Separator,
        v_flex,
    },
    prelude::*,
};

use crate::{shell::Shell, ui};

/// What `APP_FIXTURES=1` starts with.
pub(crate) fn sample_items() -> Vec<SharedString> {
    vec![
        "Quarterly report".into(),
        "Design review".into(),
        "Groceries".into(),
    ]
}

impl Shell {
    pub(crate) fn render_home(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let new_item = Button::new("new-item")
            .primary()
            .label("New Item…")
            .on_click(cx.listener(|this, _, window, cx| this.new_item(window, cx)));

        if self.items.is_empty() {
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .child(
                    Empty::new()
                        .header(
                            EmptyHeader::new()
                                .media(EmptyMedia::new().child(Icon::new(IconName::Inbox)))
                                .title(EmptyTitle::new().child("No items yet")),
                        )
                        .content(EmptyContent::new().child(new_item)),
                )
                .into_any_element();
        }

        let mut list = ui::group("Items");
        for (index, item) in self.items.iter().enumerate() {
            if index > 0 {
                list = list.child(Separator::horizontal());
            }
            list = list.child(ui::row(item.clone(), None, gpui_kit::div(), cx));
        }
        v_flex()
            .id("home")
            .flex_1()
            .overflow_y_scroll()
            .p_6()
            .gap_4()
            .child(list)
            .child(h_flex().justify_end().child(new_item))
            .into_any_element()
    }

    pub(crate) fn new_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // A dialog returns focus to whatever had it when it opened, so close the palette first.
        self.close_palette(window, cx);
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Untitled"));
        let shell = cx.entity().downgrade();
        let input = name.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let name = name.clone();
            let shell = shell.clone();
            dialog
                .title("New Item")
                .child(v_form().child(field().label("Name").child(Input::new(&name))))
                .footer(
                    DialogFooter::new()
                        .child(DialogClose::new().trigger(|button| button.label("Cancel")))
                        .child(
                            DialogAction::new()
                                .child(Button::new("create").primary().label("Create")),
                        ),
                )
                .on_ok(move |_, _, cx| {
                    let value = name.read(cx).value().trim().to_string();
                    if value.is_empty() {
                        return false; // keep the dialog open
                    }
                    shell
                        .update(cx, |this, cx| {
                            this.items.push(SharedString::from(value));
                            cx.notify();
                        })
                        .is_ok()
                })
        });
        input.update(cx, |input, cx| input.focus(window, cx));
    }
}
