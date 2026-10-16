use gpui_kit::{prelude::*, *};

use crate::tokens::Palette;

pub fn hint(text: impl Into<SharedString>, palette: Palette) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(palette.muted))
        .child(text.into())
}

pub fn caption(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}

pub fn number(text: impl Into<SharedString>, color: u32) -> Div {
    div()
        .font_family("Consolas")
        .text_size(px(13.))
        .text_color(rgb(color))
        .child(text.into())
}

/// Read-only status text. Interactive filters use selection::filter_chip.
pub fn tag(text: impl Into<SharedString>, color: u32, palette: Palette) -> Div {
    div()
        .px_2()
        .py_0p5()
        .rounded_sm()
        .text_size(px(11.))
        .text_color(rgb(color))
        .bg(rgb(palette.subtle))
        .child(text.into())
}

pub fn heading(
    eyebrow: impl Into<SharedString>,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    palette: Palette,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.))
                .text_color(rgb(palette.muted))
                .child(eyebrow.into()),
        )
        .child(
            div()
                .text_size(px(24.))
                .font_weight(FontWeight::BOLD)
                .child(title.into()),
        )
        .child(hint(subtitle, palette))
}
