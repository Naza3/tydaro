use super::*;

impl PanelContent {
    pub(super) fn chart(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("chart-periods")
                    .h(px(36.))
                    .flex_shrink_0()
                    .px_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .overflow_x_scroll()
                    .children(Period::ALL.into_iter().map(|period| {
                        control(
                            SharedString::from(format!("period-{}", period.label())),
                            period.label(),
                            period == self.period,
                        )
                        .flex_shrink_0()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_market(this.symbol, period, this.tick, cx);
                            cx.emit(ContentEvent::PeriodChanged(period));
                        }))
                        .test_support()
                    }))
                    .child(div().flex_1())
                    .when(self.period != Period::Intraday, |view| {
                        view.child(
                            control("chart-zoom-out", "−", false)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.chart_entity()
                                        .update(cx, |chart, cx| chart.zoom_step(false, cx));
                                }))
                                .test_support(),
                        )
                        .child(
                            control("chart-zoom-in", "+", false)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.chart_entity()
                                        .update(cx, |chart, cx| chart.zoom_step(true, cx));
                                }))
                                .test_support(),
                        )
                    })
                    .child(
                        control(
                            "chart-reset",
                            if self.period == Period::Intraday {
                                "全天"
                            } else {
                                "复位"
                            },
                            false,
                        )
                        .flex_shrink_0()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.chart_entity().update(cx, |chart, cx| chart.reset(cx));
                        }))
                        .test_support(),
                    ),
            )
            .when_some(self.feedback.as_ref(), |view, (message, _)| {
                view.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_color(rgb(RED))
                        .child(message.clone()),
                )
            })
            .child(div().flex_1().min_h_0().child(self.chart_entity().clone()))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(if self.period == Period::Day {
                        "单击查看数据 · ← → 逐日定位 · 双击查看当日分时 · Esc 退出定位"
                    } else if self.period == Period::Intraday {
                        "单击显示 / 隐藏定位 · 移动鼠标查看 · Esc 退出"
                    } else {
                        "单击查看数据 · 滚轮缩放 · 拖动平移 · Esc 退出定位"
                    }),
            )
            .into_any_element()
    }
}
