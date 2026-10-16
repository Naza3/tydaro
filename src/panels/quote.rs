use super::*;

impl PanelContent {
    pub(super) fn quote(&self) -> AnyElement {
        let q = market::quote(self.symbol, self.tick);
        let price_color = |price| movement(price - q.previous_close);
        div()
            .id("quote-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .px_2()
                    .h(px(40.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .text_color(movement(q.change))
                    .child(
                        div()
                            .text_size(px(22.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{:.3}", q.price)),
                    )
                    .child(format!("{:+.3}  {:+.2}%", q.change, q.change_pct)),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .child(metric_colored(
                        "最高",
                        format!("{:.3}", q.high),
                        price_color(q.high),
                        "开盘",
                        format!("{:.3}", q.open),
                        price_color(q.open),
                    ))
                    .child(metric_colored(
                        "最低",
                        format!("{:.3}", q.low),
                        price_color(q.low),
                        "昨收",
                        format!("{:.3}", q.previous_close),
                        rgb(INK),
                    ))
                    .child(metric(
                        "成交量",
                        compact(q.volume as f64),
                        "成交额",
                        compact(q.turnover),
                    ))
                    .child(metric(
                        "量比",
                        format!("{:.2}", q.volume_ratio),
                        "换手率",
                        format!("{:.2}%", q.turnover_rate),
                    ))
                    .child(metric(
                        "振幅",
                        format!("{:.2}%", (q.high - q.low) / q.previous_close * 100.),
                        "均价",
                        format!("{:.3}", q.turnover / q.volume.max(1) as f64),
                    ))
                    .child(metric("涨停", "—".into(), "跌停", "—".into()))
                    .child(metric_colored(
                        "主动买",
                        compact(q.buy_volume as f64),
                        rgb(RED),
                        "主动卖",
                        compact(q.sell_volume as f64),
                        rgb(DOWN),
                    ))
                    .child(metric(
                        "市盈(TTM)",
                        format!("{:.2}", q.pe_ttm),
                        "每股盈利",
                        format!("{:.3}", q.price / q.pe_ttm),
                    ))
                    .child(metric(
                        "总市值",
                        compact(q.market_cap),
                        "流通值",
                        compact(q.float_market_cap),
                    ))
                    .child(metric(
                        "总股本",
                        compact(q.total_shares as f64),
                        "流通股",
                        compact(q.float_shares as f64),
                    )),
            )
            .child(
                div()
                    .px_2()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(format!("{} · 量 / 股本：股 · 模拟", currency(self.symbol))),
            )
            .into_any_element()
    }
}
