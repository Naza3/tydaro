//! Controlled selectors: callers own the selected value and handle changes.
use gpui_kit::{
    component::{
        Selectable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};

use crate::tokens::Palette;

pub fn list_row(id: impl Into<ElementId>, selected: bool, palette: Palette) -> Button {
    Button::new(id)
        .ghost()
        .w_full()
        .justify_start()
        .text_size(px(13.))
        .text_color(rgb(palette.text))
        .bg(rgb(if selected {
            palette.subtle
        } else {
            palette.surface
        }))
        .selected(selected)
}

/// An outlined choice with caller-supplied content, suitable for labeled rows
/// or segmented controls. The caller supplies its selection semantics and events.
pub fn choice(id: impl Into<ElementId>, selected: bool, palette: Palette) -> Button {
    Button::new(id)
        .ghost()
        .w_full()
        .border_1()
        .border_color(rgb(if selected {
            palette.accent
        } else {
            palette.border
        }))
        .bg(rgb(if selected {
            palette.subtle
        } else {
            palette.surface
        }))
        .rounded_md()
}

pub fn filter_chip(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
    palette: Palette,
) -> Button {
    Button::new(id)
        .ghost()
        .h(px(33.))
        .px_3()
        .border_1()
        .border_color(rgb(if selected {
            palette.accent
        } else {
            palette.border
        }))
        .bg(rgb(if selected {
            palette.subtle
        } else {
            palette.surface
        }))
        .rounded_full()
        .label(label)
}
