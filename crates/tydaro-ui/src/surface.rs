use gpui_kit::{prelude::*, *};

use crate::tokens::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceStyle {
    /// Bordered page section with clipped children.
    Panel,
    /// Bordered compact section; children decide padding and clipping.
    Compact,
    Plain,
    LeadingRule,
}

pub fn surface(palette: Palette, style: SurfaceStyle) -> Div {
    let base = div().min_w_0().bg(rgb(palette.surface));
    match style {
        SurfaceStyle::Panel => base
            .border_1()
            .border_color(rgb(palette.border))
            .rounded_lg()
            .overflow_hidden(),
        SurfaceStyle::Compact => base
            .text_color(rgb(palette.text))
            .border_1()
            .border_color(rgb(palette.border))
            .rounded_sm(),
        SurfaceStyle::Plain => base.text_color(rgb(palette.text)).rounded_lg(),
        SurfaceStyle::LeadingRule => base
            .text_color(rgb(palette.text))
            .border_l_2()
            .border_color(rgb(palette.border)),
    }
}

pub fn page(palette: Palette) -> Div {
    div()
        .size_full()
        .min_h_0()
        .flex()
        .flex_col()
        .gap_4()
        .p_5()
        .bg(rgb(palette.background))
        .text_size(px(14.))
        .text_color(rgb(palette.text))
        .overflow_hidden()
}
