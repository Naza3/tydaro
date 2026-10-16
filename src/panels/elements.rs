//! Small panel-only presentation helpers.
use super::*;

pub(super) fn control(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    active: bool,
) -> Stateful<Div> {
    div()
        .id(id)
        .text_size(px(13.))
        .px_2()
        .py_1()
        .rounded(px(4.))
        .cursor_pointer()
        .bg(rgb(if active { 0xe8f7f6 } else { 0xffffff }))
        .text_color(rgb(if active { TEAL } else { INK }))
        .hover(|s| s.bg(rgb(0xeef4f7)))
        .child(text.into())
}
pub(super) fn label(text: &'static str) -> Div {
    div()
        .mb_1()
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(text)
}
pub(super) fn badge(text: &'static str) -> Div {
    div()
        .px_1()
        .py(px(2.))
        .rounded(px(3.))
        .bg(rgb(0xf0f5f8))
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(text)
}
pub(super) fn movement(change: f64) -> Rgba {
    rgb(if change >= 0. { RED } else { DOWN })
}
pub(super) fn currency(symbol: Symbol) -> &'static str {
    symbol.currency()
}
pub(super) fn compact(value: f64) -> String {
    if value.abs() >= 1e12 {
        format!("{:.2} 万亿", value / 1e12)
    } else if value.abs() >= 1e8 {
        format!("{:.2} 亿", value / 1e8)
    } else if value.abs() >= 1e4 {
        format!("{:.2} 万", value / 1e4)
    } else {
        format!("{:.0}", value)
    }
}
pub(super) fn metric(
    left: &'static str,
    left_value: String,
    right: &'static str,
    right_value: String,
) -> Div {
    metric_colored(left, left_value, rgb(INK), right, right_value, rgb(INK))
}
pub(super) fn metric_colored(
    left: &'static str,
    left_value: String,
    left_color: Rgba,
    right: &'static str,
    right_value: String,
    right_color: Rgba,
) -> Div {
    div().h(px(24.)).flex().items_center().gap_2().children(
        [
            (left, left_value, left_color),
            (right, right_value, right_color),
        ]
        .into_iter()
        .map(|(label, value, color)| {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_between()
                .gap_1()
                .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(label))
                .child(div().min_w_0().truncate().text_color(color).child(value))
        }),
    )
}
pub(super) fn dense_columns(columns: &[(&'static str, f32)], right: bool) -> Div {
    div()
        .h(px(24.))
        .px_2()
        .flex()
        .items_center()
        .flex_shrink_0()
        .bg(rgb(0xf5f7fa))
        .border_b_1()
        .border_color(rgb(LINE))
        .text_color(rgb(MUTED))
        .children(columns.iter().enumerate().map(|(index, (title, width))| {
            div()
                .w(px(*width))
                .flex_shrink_0()
                .when(right && index > 0, |d| d.text_right())
                .child(*title)
        }))
}
pub(super) fn amount_cell(amount: f64, ratio: f32, color: u32, width: f32) -> Div {
    div()
        .relative()
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_end()
        .px_1()
        .child(
            div()
                .absolute()
                .right_0()
                .top(px(2.))
                .bottom(px(2.))
                .w(relative(ratio.clamp(0., 1.)))
                .bg(rgb(color))
                .opacity(0.10),
        )
        .child(
            div()
                .relative()
                .text_color(rgb(color))
                .child(if amount == 0. {
                    "—".into()
                } else {
                    compact(amount)
                }),
        )
}
pub(super) fn table_header(columns: &[(&'static str, f32)]) -> Div {
    div()
        .h(px(27.))
        .px_3()
        .flex()
        .items_center()
        .bg(rgb(0xf8fafc))
        .text_color(rgb(MUTED))
        .children(
            columns
                .iter()
                .map(|(title, width)| div().w(px(*width)).flex_shrink_0().child(*title)),
        )
}
