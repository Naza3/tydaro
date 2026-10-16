//! Button geometry is independent from its action and the state it changes.
use gpui_kit::{
    component::{
        Sizable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionSize {
    /// Existing dense page toolbars, with GPUI's small hit target.
    Small,
    Compact,
    Comfortable,
    Inline,
}

/// A labeled native button. Native keyboard, disabled and accessibility behavior
/// remains with GPUI; callers can attach primary/ghost/selected semantics.
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label)
}

pub fn action(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    size: ActionSize,
) -> Button {
    let button = button(id, label);
    match size {
        ActionSize::Small => button.small().text_size(px(12.)),
        ActionSize::Compact => button.text_size(px(13.)).h(px(30.)).rounded_sm().px_2(),
        ActionSize::Comfortable => button.text_size(px(13.)).h(px(38.)).rounded_lg().px_4(),
        ActionSize::Inline => button
            .text_size(px(13.))
            .ghost()
            .h(px(34.))
            .rounded_md()
            .px_3(),
    }
}
