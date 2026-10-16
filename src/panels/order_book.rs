use super::*;

impl PanelContent {
    pub(super) fn order_book(&self, cx: &mut Context<Self>) -> AnyElement {
        let (bids, asks) = market::order_book(self.symbol, self.tick);
        let buy_total: u64 = bids.iter().map(|l| l.size).sum();
        let sell_total: u64 = asks.iter().map(|l| l.size).sum();
        let ratio = buy_total as f32 / (buy_total + sell_total).max(1) as f32;
        let max_size = bids
            .iter()
            .chain(asks.iter())
            .map(|l| l.size)
            .max()
            .unwrap_or(1) as f32;
        let pending: Vec<_> = market::market_orders(self.symbol, self.tick)
            .into_iter()
            .filter(|order| order.canceled_at.is_none())
            .collect();
        let q = market::quote(self.symbol, self.tick);
        let mut depth = div()
            .id("book-scroll")
            .flex_1()
            .min_h(px(40.))
            .overflow_scroll();
        if self.book_queue {
            let max = pending.iter().map(|order| order.size).max().unwrap_or(1) as f32;
            depth = depth
                .child(dense_columns(
                    &[
                        ("方向", 36.),
                        ("价格", 75.),
                        ("委托量（股）", 95.),
                        ("时间", 70.),
                    ],
                    true,
                ))
                .children(pending.iter().map(|order| {
                    div()
                        .h(px(24.))
                        .px_2()
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xf3f5f7))
                        .child(
                            div()
                                .w(px(36.))
                                .flex_shrink_0()
                                .text_color(rgb(if order.buy { RED } else { DOWN }))
                                .child(if order.buy { "买" } else { "卖" }),
                        )
                        .child(
                            div()
                                .w(px(75.))
                                .flex_shrink_0()
                                .child(format!("{:.3}", order.price)),
                        )
                        .child(amount_cell(
                            order.size as f64,
                            order.size as f32 / max,
                            if order.buy { RED } else { DOWN },
                            95.,
                        ))
                        .child(
                            div()
                                .w(px(70.))
                                .text_right()
                                .text_color(rgb(MUTED))
                                .child(order.time.clone()),
                        )
                }));
        } else {
            depth = depth
                .child(book_section("卖盘", DOWN))
                .children(asks.iter().enumerate().map(|(index, level)| {
                    book_row(
                        format!("卖 {}", index + 1),
                        level.price,
                        level.size,
                        max_size,
                        DOWN,
                    )
                }))
                .child(book_section("买盘", RED))
                .children(bids.iter().enumerate().map(|(index, level)| {
                    book_row(
                        format!("买 {}", index + 1),
                        level.price,
                        level.size,
                        max_size,
                        RED,
                    )
                }));
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(24.))
                    .px_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(
                        control("book-depth", "买卖十档", !self.book_queue)
                            .py(px(2.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.book_queue = false;
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .child(
                        control("book-queue", "委托队列", self.book_queue)
                            .py(px(2.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.book_queue = true;
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(format!("股 · {}", currency(self.symbol))),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .h(px(24.))
                    .items_center()
                    .flex_shrink_0()
                    .flex()
                    .justify_between()
                    .text_size(px(12.))
                    .child(
                        div()
                            .text_color(rgb(RED))
                            .child(format!("买盘 {:.1}%", ratio * 100.)),
                    )
                    .child(
                        div()
                            .text_color(rgb(DOWN))
                            .child(format!("卖盘 {:.1}%", (1. - ratio) * 100.)),
                    ),
            )
            .child(
                div()
                    .mx_2()
                    .h(px(2.))
                    .flex_shrink_0()
                    .flex()
                    .bg(rgb(DOWN))
                    .child(div().h_full().w(relative(ratio)).bg(rgb(RED))),
            )
            .child(
                div()
                    .id("best-bid-ask")
                    .mx_2()
                    .flex_shrink_0()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .px_1()
                            .flex()
                            .items_center()
                            .justify_between()
                            .bg(rgb(0xffeff3))
                            .text_color(rgb(RED))
                            .child("买一")
                            .child(format!("{:.3}", bids[0].price)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .px_1()
                            .flex()
                            .items_center()
                            .justify_between()
                            .bg(rgb(0xe8f8f5))
                            .text_color(rgb(DOWN))
                            .child("卖一")
                            .child(format!("{:.3}", asks[0].price)),
                    )
                    .test_support(),
            )
            .when(!self.book_queue, |d| {
                d.child(
                    div()
                        .h(px(24.))
                        .px_2()
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .text_color(rgb(MUTED))
                        .border_b_1()
                        .border_color(rgb(LINE))
                        .child(div().w(px(36.)).child("档位"))
                        .child(div().flex_1().text_right().child("价格"))
                        .child(div().w(px(90.)).text_right().child("数量（股）")),
                )
            })
            .child(depth)
            .child(
                div()
                    .px_2()
                    .h(px(24.))
                    .items_center()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .flex()
                    .justify_between()
                    .child(format!("均价 {:.3}", q.turnover / q.volume.max(1) as f64))
                    .child(
                        div()
                            .text_color(rgb(MUTED))
                            .child(format!("挂单 {} 笔", pending.len())),
                    ),
            )
            .into_any_element()
    }
}

fn book_section(title: &'static str, color: u32) -> Div {
    div()
        .h(px(24.))
        .px_2()
        .flex()
        .items_center()
        .bg(rgb(0xf7f9fb))
        .text_size(px(12.))
        .text_color(rgb(color))
        .border_t_1()
        .border_color(rgb(LINE))
        .child(title)
}
fn book_row(label: String, price: f64, quantity: u64, max: f32, color: u32) -> Div {
    div()
        .relative()
        .h(px(24.))
        .mx_2()
        .flex()
        .items_center()
        .child(
            div()
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .w(relative(quantity as f32 / max * 0.65))
                .bg(rgb(color))
                .opacity(0.07),
        )
        .child(div().w(px(36.)).text_color(rgb(color)).child(label))
        .child(
            div()
                .flex_1()
                .text_right()
                .text_color(rgb(color))
                .child(format!("{price:.3}")),
        )
        .child(div().w(px(90.)).text_right().child(quantity.to_string()))
}
