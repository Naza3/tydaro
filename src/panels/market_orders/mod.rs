//! Read-only market orders, independent of simulated account orders.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
struct OrderRange {
    min_amount: f64,
    max_amount: f64,
    min_quantity: f64,
    max_quantity: f64,
}
impl OrderRange {
    fn parse(saved: &OrderFilterSnapshot) -> Result<Self, &'static str> {
        fn bound(value: &str, fallback: f64) -> Result<f64, &'static str> {
            if value.trim().is_empty() {
                return Ok(fallback);
            }
            value
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && *value >= 0.)
                .ok_or("筛选值须为非负数字")
        }
        let range = Self {
            min_amount: bound(&saved.min_amount, 0.)? * 10_000.,
            max_amount: bound(&saved.max_amount, f64::MAX / 10_000.)? * 10_000.,
            min_quantity: bound(&saved.min_quantity, 0.)?,
            max_quantity: bound(&saved.max_quantity, f64::MAX)?,
        };
        if range.min_amount > range.max_amount || range.min_quantity > range.max_quantity {
            return Err("最低值不能大于最高值");
        }
        if !range.min_amount.is_finite() || !range.max_amount.is_finite() {
            return Err("筛选金额超出范围");
        }
        Ok(range)
    }
    fn contains(self, order: &market::MarketOrder) -> bool {
        order.amount >= self.min_amount
            && order.amount <= self.max_amount
            && order.size as f64 >= self.min_quantity
            && order.size as f64 <= self.max_quantity
    }
}

impl PanelContent {
    pub(super) fn filter_snapshot(&self, cx: &App) -> OrderFilterSnapshot {
        OrderFilterSnapshot {
            enabled: self.filter_enabled,
            min_amount: self.min_amount().read(cx).value().to_string(),
            max_amount: self.max_amount().read(cx).value().to_string(),
            min_quantity: self.min_quantity().read(cx).value().to_string(),
            max_quantity: self.max_quantity().read(cx).value().to_string(),
            direction: self.filter_direction,
        }
    }

    pub(super) fn order_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let filter = self.filter_snapshot(cx);
        let range = OrderRange::parse(&filter);
        let orders: Vec<_> = market::market_orders(self.symbol, self.tick)
            .into_iter()
            .filter(|order| {
                (filter.direction == 0 || (filter.direction == 1) == order.buy)
                    && (!filter.enabled || range.as_ref().is_ok_and(|range| range.contains(order)))
            })
            .collect();
        let pending: Vec<_> = orders
            .iter()
            .filter(|order| order.canceled_at.is_none())
            .collect();
        let canceled: Vec<_> = orders
            .iter()
            .filter(|order| order.canceled_at.is_some())
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("order-filter-toolbar")
                    .px_1()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_wrap()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(
                        control(
                            "market-filter-enabled",
                            if self.filter_enabled {
                                "☑ 筛选"
                            } else {
                                "☐ 筛选"
                            },
                            self.filter_enabled,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.filter_enabled = !this.filter_enabled;
                            cx.notify();
                        }))
                        .test_support(),
                    )
                    .child(
                        div()
                            .px_1()
                            .text_color(rgb(MUTED))
                            .child(format!("金额（万 {}）", currency(self.symbol))),
                    )
                    .child(
                        Input::new(self.min_amount())
                            .id("market-min-amount")
                            .small()
                            .w(px(66.)),
                    )
                    .child("–")
                    .child(
                        Input::new(self.max_amount())
                            .id("market-max-amount")
                            .small()
                            .w(px(66.)),
                    )
                    .child(div().px_1().text_color(rgb(MUTED)).child("数量（股）"))
                    .child(
                        Input::new(self.min_quantity())
                            .id("market-min-quantity")
                            .small()
                            .w(px(66.)),
                    )
                    .child("–")
                    .child(
                        Input::new(self.max_quantity())
                            .id("market-max-quantity")
                            .small()
                            .w(px(66.)),
                    )
                    .children([(0, "全部"), (1, "买单"), (2, "卖单")].into_iter().map(
                        |(direction, label)| {
                            control(
                                SharedString::from(format!("market-direction-{direction}")),
                                label,
                                self.filter_direction == direction,
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.filter_direction = direction;
                                cx.notify();
                            }))
                            .test_support()
                        },
                    )),
            )
            .when(self.filter_enabled && range.is_err(), |d| {
                d.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_color(rgb(RED))
                        .child(range.as_ref().unwrap_err().to_string()),
                )
            })
            .child(
                div()
                    .id("market-orders-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_x_scroll()
                    .child(
                        div()
                            .h_full()
                            .min_w(px(970.))
                            .flex()
                            .child(order_detail_table(
                                "pending-market-orders",
                                "挂单",
                                &pending,
                                false,
                            ))
                            .child(order_detail_table(
                                "canceled-market-orders",
                                "撤单",
                                &canceled,
                                true,
                            )),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child("市场委托为模拟数据 · 本地账户委托请查看“当日订单”"),
            )
            .into_any_element()
    }
}

fn order_detail_table(
    id: &'static str,
    title: &'static str,
    orders: &[&market::MarketOrder],
    canceled: bool,
) -> Div {
    let max = orders.iter().map(|order| order.amount).fold(1., f64::max);
    div()
        .h_full()
        .flex_1()
        .min_w(px(480.))
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(rgb(LINE))
        .child(
            div()
                .h(px(22.))
                .px_2()
                .flex()
                .items_center()
                .justify_between()
                .flex_shrink_0()
                .bg(rgb(0xf2f6fa))
                .child(div().font_weight(FontWeight::MEDIUM).child(title))
                .child(
                    div()
                        .text_color(rgb(MUTED))
                        .child(format!("{} 笔", orders.len())),
                ),
        )
        .child(dense_columns(
            &[
                (
                    if canceled {
                        "撤单时间"
                    } else {
                        "委托时间"
                    },
                    65.,
                ),
                ("方向", 40.),
                ("委托价格", 70.),
                ("股数", 60.),
                ("委托金额", 85.),
                (if canceled { "委托时间" } else { "状态" }, 90.),
            ],
            false,
        ))
        .child(
            div()
                .id(id)
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .when(orders.is_empty(), |d| {
                    d.child(
                        div()
                            .p_3()
                            .text_color(rgb(MUTED))
                            .child("没有满足条件的委托"),
                    )
                })
                .children(orders.iter().map(|order| {
                    let color = if order.buy { RED } else { DOWN };
                    div()
                        .h(px(24.))
                        .px_2()
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xf0f3f6))
                        .child(
                            div()
                                .w(px(65.))
                                .flex_shrink_0()
                                .text_color(rgb(MUTED))
                                .child(order.canceled_at.as_ref().unwrap_or(&order.time).clone()),
                        )
                        .child(
                            div()
                                .w(px(40.))
                                .flex_shrink_0()
                                .text_color(rgb(color))
                                .child(match (canceled, order.buy) {
                                    (true, true) => "撤买",
                                    (true, false) => "撤卖",
                                    (false, true) => "委买",
                                    (false, false) => "委卖",
                                }),
                        )
                        .child(
                            div()
                                .w(px(70.))
                                .flex_shrink_0()
                                .text_color(rgb(color))
                                .child(format!("{:.3}", order.price)),
                        )
                        .child(
                            div()
                                .w(px(60.))
                                .flex_shrink_0()
                                .text_color(rgb(color))
                                .child(order.size.to_string()),
                        )
                        .child(amount_cell(
                            order.amount,
                            (order.amount / max) as f32,
                            color,
                            85.,
                        ))
                        .child(
                            div()
                                .w(px(90.))
                                .flex_shrink_0()
                                .px_2()
                                .text_color(rgb(MUTED))
                                .child(if canceled {
                                    order.time.clone()
                                } else {
                                    "排队中".into()
                                }),
                        )
                })),
        )
}

#[cfg(test)]
mod tests;
