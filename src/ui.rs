//! Settings-style layout. gpui-component's own `setting` renderers are crate-private.

use gpui_kit::{
    App, FontWeight, IntoElement, ParentElement, SharedString, StyleRefinement, Styled,
    component::{
        ActiveTheme as _,
        group_box::{GroupBox, GroupBoxVariants as _},
        h_flex, v_flex,
    },
    div, px,
};

pub fn group(title: impl Into<SharedString>) -> GroupBox {
    GroupBox::new()
        .fill()
        .title(title.into())
        .title_style(
            StyleRefinement::default()
                .text_sm()
                .font_weight(FontWeight::MEDIUM),
        )
        .content_style(StyleRefinement::default().gap_0().px_3().py_0())
}

pub fn row(
    label: impl Into<SharedString>,
    detail: Option<SharedString>,
    control: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .min_h(px(40.0))
        .py_2()
        .gap_4()
        .justify_between()
        .child(
            v_flex()
                .gap_0p5()
                .child(div().text_base().child(label.into()))
                .children(detail.map(|detail| {
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(detail)
                })),
        )
        .child(control)
}
