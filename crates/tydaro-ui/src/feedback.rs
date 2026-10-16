use gpui_kit::{prelude::*, *};

use crate::{text, tokens::Palette};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackTone {
    Neutral,
    Success,
    Error,
}

pub fn inline(text: impl Into<SharedString>, tone: FeedbackTone, palette: Palette) -> Div {
    let color = match tone {
        FeedbackTone::Neutral => palette.muted,
        FeedbackTone::Success => palette.success,
        FeedbackTone::Error => palette.error,
    };
    text::hint(text, palette)
        .min_h(px(20.))
        .text_color(rgb(color))
}
