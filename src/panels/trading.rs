//! Simulated account forms and views.
use super::*;

impl Side {
    pub(super) fn label(self) -> &'static str {
        if self == Self::Buy {
            "买入"
        } else {
            "卖出"
        }
    }
    pub(super) fn color(self) -> u32 {
        if self == Self::Buy { RED } else { DOWN }
    }
}

impl PanelContent {
    pub(super) fn submit_order(&mut self, cx: &mut Context<Self>) {
        let price = self.price().read(cx).value().trim().parse::<f64>();
        let quantity = self.quantity().read(cx).value().trim().parse::<u64>();
        let result = match (price, quantity) {
            (Ok(price), Ok(quantity)) => cx.update_global::<DemoBroker, _>(|broker, _| {
                broker.submit(self.symbol, self.side, price, quantity)
            }),
            _ => Err("请输入有效价格和整数股数。"),
        };
        self.feedback = Some(match result {
            Ok(id) => (
                format!("模拟委托 #{id} 已记录。在“当日订单”中可模拟成交或撤单。"),
                true,
            ),
            Err(message) => (message.into(), false),
        });
        cx.emit(ContentEvent::OrderSubmitted);
        cx.notify();
    }

    pub(super) fn order_entry(&self, cx: &mut Context<Self>) -> AnyElement {
        let broker = cx.global::<DemoBroker>();
        let available_cash = broker.available_cash(self.symbol);
        let available_shares = broker.available_shares(self.symbol);
        let price = self.price().read(cx).value().parse::<f64>().unwrap_or(0.);
        let qty = self.quantity().read(cx).value().parse::<u64>().unwrap_or(0);
        let amount = if price.is_finite() && price > 0. {
            price * qty as f64
        } else {
            0.
        };
        div()
            .id("entry-scroll")
            .size_full()
            .overflow_y_scroll()
            .p_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mb_3()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                        "{} {}",
                        self.symbol.code(),
                        self.symbol.name()
                    )))
                    .child(badge("本地模拟")),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .mb_3()
                    .children([Side::Buy, Side::Sell].into_iter().map(|side| {
                        control(
                            if side == Side::Buy {
                                "order-buy"
                            } else {
                                "order-sell"
                            },
                            side.label(),
                            self.side == side,
                        )
                        .flex_1()
                        .text_center()
                        .text_color(rgb(side.color()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.side = side;
                            this.feedback = None;
                            cx.notify();
                        }))
                        .test_support()
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        div().flex_1().min_w(px(125.)).child(label("限价")).child(
                            Input::new(self.price())
                                .id("order-price")
                                .small()
                                .w_full()
                                .suffix(currency(self.symbol)),
                        ),
                    )
                    .child(
                        div().flex_1().min_w(px(125.)).child(label("数量")).child(
                            Input::new(self.quantity())
                                .id("order-quantity")
                                .small()
                                .w_full()
                                .suffix("股"),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mt_2()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(format!(
                        "可用 {:.2} {}",
                        available_cash,
                        currency(self.symbol)
                    ))
                    .child(format!("可卖 {} 股", available_shares)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mt_3()
                    .child(label("委托金额"))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                        "{:.2} {}",
                        amount,
                        currency(self.symbol)
                    ))),
            )
            .child(
                div()
                    .id("submit-order")
                    .px_2()
                    .py_1()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .child(format!("提交模拟{}委托", self.side.label()))
                    .mt_3()
                    .w_full()
                    .text_center()
                    .bg(rgb(self.side.color()))
                    .text_color(rgb(0xffffff))
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|this, _, _, cx| this.submit_order(cx)))
                    .test_support(),
            )
            .when_some(self.feedback.as_ref(), |d, (message, success)| {
                d.child(
                    div()
                        .id("order-feedback")
                        .mt_2()
                        .p_2()
                        .rounded_md()
                        .text_size(px(12.))
                        .line_height(relative(1.5))
                        .bg(rgb(if *success { 0xe9f7f4 } else { 0xffeff2 }))
                        .text_color(rgb(if *success { TEAL } else { RED }))
                        .child(message.clone())
                        .test_support(),
                )
            })
            .child(
                div()
                    .mt_2()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child("仅本地演示 · 手动模拟成交 · 重启后重置账户"),
            )
            .into_any_element()
    }

    pub(super) fn orders(&self, cx: &mut Context<Self>) -> AnyElement {
        let orders = cx.global::<DemoBroker>().orders.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(31.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(badge("模拟账户"))
                    .child(
                        div()
                            .text_color(rgb(MUTED))
                            .child(format!("{} 笔委托", orders.len())),
                    ),
            )
            .child(
                div()
                    .id("orders-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .child(div().min_w(px(620.)).child(table_header(&[
                        ("标的", 115.),
                        ("方向", 55.),
                        ("委托价", 90.),
                        ("数量", 65.),
                        ("状态", 105.),
                        ("操作", 150.),
                    ])))
                    .children(orders.into_iter().rev().map(|order| {
                        let id = order.id;
                        div()
                            .min_w(px(620.))
                            .h(px(35.))
                            .px_3()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0xf1f3f5))
                            .child(div().w(px(115.)).child(format!(
                                "{}  #{}",
                                order.symbol.code(),
                                order.id
                            )))
                            .child(
                                div()
                                    .w(px(55.))
                                    .text_color(rgb(order.side.color()))
                                    .child(order.side.label()),
                            )
                            .child(div().w(px(90.)).child(format!("{:.2}", order.price)))
                            .child(div().w(px(65.)).child(order.quantity.to_string()))
                            .child(
                                div()
                                    .w(px(105.))
                                    .text_color(rgb(if order.status == OrderStatus::Pending {
                                        0xe39e2c
                                    } else {
                                        MUTED
                                    }))
                                    .child(order.status.label()),
                            )
                            .when(order.status == OrderStatus::Pending, |d| {
                                d.child(
                                    div()
                                        .flex()
                                        .gap_1()
                                        .child(
                                            control(
                                                SharedString::from(format!("fill-{id}")),
                                                "模拟成交",
                                                true,
                                            )
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.update_global::<DemoBroker, _>(|broker, _| {
                                                    broker.fill(id)
                                                });
                                                cx.emit(ContentEvent::OrderSubmitted);
                                                cx.notify();
                                            }))
                                            .test_support(),
                                        )
                                        .child(
                                            control(
                                                SharedString::from(format!("cancel-{id}")),
                                                "撤单",
                                                false,
                                            )
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.update_global::<DemoBroker, _>(|broker, _| {
                                                    broker.cancel(id)
                                                });
                                                cx.emit(ContentEvent::OrderSubmitted);
                                                cx.notify();
                                            }))
                                            .test_support(),
                                        ),
                                )
                            })
                    })),
            )
            .into_any_element()
    }

    pub(super) fn positions(&self, cx: &mut Context<Self>) -> AnyElement {
        let positions = cx.global::<DemoBroker>().positions.clone();
        let (usd, hkd, cny) = {
            let broker = cx.global::<DemoBroker>();
            (broker.usd, broker.hkd, broker.cny)
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("positions-balances")
                    .h(px(36.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_4()
                    .overflow_x_scroll()
                    .child(badge("模拟持仓"))
                    .child(format!("现金 {usd:.2} USD"))
                    .child(div().text_color(rgb(MUTED)).child(format!("{hkd:.2} HKD")))
                    .child(div().text_color(rgb(MUTED)).child(format!("{cny:.2} CNY"))),
            )
            .child(
                div()
                    .id("positions-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .child(div().min_w(px(605.)).child(table_header(&[
                        ("股票", 110.),
                        ("持有", 65.),
                        ("成本", 85.),
                        ("现价", 85.),
                        ("市值", 100.),
                        ("浮动盈亏", 130.),
                    ])))
                    .children(positions.into_iter().map(|position| {
                        let q = market::quote(position.symbol, self.tick);
                        let profit = (q.price - position.cost) * position.quantity as f64;
                        div()
                            .id(SharedString::from(format!(
                                "position-{}",
                                position.symbol.code()
                            )))
                            .min_w(px(605.))
                            .h(px(36.))
                            .px_3()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0xf1f3f5))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0xf5fafb)))
                            .child(div().w(px(110.)).child(format!(
                                "{} {}",
                                position.symbol.code(),
                                position.symbol.name()
                            )))
                            .child(div().w(px(65.)).child(position.quantity.to_string()))
                            .child(div().w(px(85.)).child(format!("{:.2}", position.cost)))
                            .child(
                                div()
                                    .w(px(85.))
                                    .text_color(movement(q.change))
                                    .child(format!("{:.2}", q.price)),
                            )
                            .child(
                                div()
                                    .w(px(100.))
                                    .child(format!("{:.2}", q.price * position.quantity as f64)),
                            )
                            .child(
                                div()
                                    .w(px(130.))
                                    .text_color(movement(profit))
                                    .child(format!("{profit:+.2} {}", currency(position.symbol))),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_symbol(position.symbol, cx)
                            }))
                            .test_support()
                    })),
            )
            .into_any_element()
    }
}
