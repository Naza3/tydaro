use super::*;

impl PanelContent {
    pub(super) fn trades(&self) -> AnyElement {
        let q = market::quote(self.symbol, self.tick);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(24.))
                    .px_2()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .text_color(rgb(MUTED))
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(div().w(px(58.)).child("时间"))
                    .child(
                        div()
                            .flex_1()
                            .text_right()
                            .child(format!("价格 {}", currency(self.symbol))),
                    )
                    .child(div().w(px(62.)).text_right().child("量（股）"))
                    .child(div().w(px(32.)).text_right().child("笔数")),
            )
            .child(
                div()
                    .id("trades-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(
                        market::trades(self.symbol, self.tick)
                            .into_iter()
                            .enumerate()
                            .map(|(index, trade)| {
                                div()
                                    .h(px(24.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .bg(rgb(if index % 5 == 0 { 0xf5f8fb } else { 0xffffff }))
                                    .child(
                                        div().w(px(58.)).text_color(rgb(MUTED)).child(trade.time),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_right()
                                            .text_color(movement(trade.price - q.previous_close))
                                            .child(format!("{:.3}", trade.price)),
                                    )
                                    .child(
                                        div()
                                            .w(px(62.))
                                            .text_right()
                                            .text_color(rgb(if trade.buy { RED } else { DOWN }))
                                            .child(trade.size.to_string()),
                                    )
                                    .child(
                                        div()
                                            .w(px(32.))
                                            .text_right()
                                            .text_color(rgb(MUTED))
                                            .child(trade.count.to_string()),
                                    )
                            }),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn trade_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let executions: Vec<_> = market::executions(self.symbol, self.tick)
            .into_iter()
            .filter(|trade| !self.large_trades_only || trade.amount >= 100_000.)
            .collect();
        let max = executions
            .iter()
            .map(|trade| {
                trade
                    .amount
                    .max(trade.buy_order_amount)
                    .max(trade.sell_order_amount)
            })
            .fold(1., f64::max);
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
                        control("flow-all", "全部成交", !self.large_trades_only)
                            .py(px(2.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.large_trades_only = false;
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .child(
                        control("flow-large", "大单 ≥ 10万", self.large_trades_only)
                            .py(px(2.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.large_trades_only = true;
                                cx.notify();
                            }))
                            .test_support(),
                    ),
            )
            .child(
                div()
                    .id("executions-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .child(
                        div()
                            .min_w(px(354.))
                            .child(dense_columns(
                                &[
                                    ("时间", 64.),
                                    ("价格", 66.),
                                    ("金额", 76.),
                                    ("买单", 66.),
                                    ("卖单", 66.),
                                ],
                                true,
                            ))
                            .when(executions.is_empty(), |d| {
                                d.child(
                                    div()
                                        .p_3()
                                        .text_color(rgb(MUTED))
                                        .child("当前没有满足条件的成交"),
                                )
                            })
                            .children(executions.iter().map(|trade| {
                                div()
                                    .h(px(24.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .border_b_1()
                                    .border_color(rgb(0xf3f5f7))
                                    .child(
                                        div()
                                            .w(px(64.))
                                            .flex_shrink_0()
                                            .text_color(rgb(MUTED))
                                            .child(trade.time.clone()),
                                    )
                                    .child(
                                        div()
                                            .w(px(66.))
                                            .flex_shrink_0()
                                            .text_right()
                                            .text_color(rgb(if trade.buy { RED } else { DOWN }))
                                            .child(format!("{:.3}", trade.price)),
                                    )
                                    .child(amount_cell(
                                        trade.amount,
                                        (trade.amount / max) as f32,
                                        if trade.buy { RED } else { DOWN },
                                        76.,
                                    ))
                                    .child(amount_cell(
                                        trade.buy_order_amount,
                                        (trade.buy_order_amount / max) as f32,
                                        RED,
                                        66.,
                                    ))
                                    .child(amount_cell(
                                        trade.sell_order_amount,
                                        (trade.sell_order_amount / max) as f32,
                                        DOWN,
                                        66.,
                                    ))
                            })),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(format!(
                        "{} · {} 笔 · 买单 / 卖单为对应委托金额",
                        currency(self.symbol),
                        executions.len()
                    )),
            )
            .into_any_element()
    }
}
