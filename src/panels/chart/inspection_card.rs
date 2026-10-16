//! Data cards for the currently inspected candle or minute.
use super::*;

pub(super) fn intraday_card_on_left(x: f32, geometry: Geometry) -> bool {
    x >= geometry.left + geometry.width / 2.
}

pub(super) fn intraday_inspection_card(
    details: IntradayDetails,
    geometry: Geometry,
    selected_x: f32,
    currency: &'static str,
    cx: &mut Context<MarketChart>,
) -> impl IntoElement + use<> {
    let color = match details.change {
        Some(change) if change > 0.000_001 => UP,
        Some(change) if change < -0.000_001 => DOWN,
        _ => MUTED,
    };
    let fields = [
        ("latest", "最新", format!("{:.3}", details.latest), color),
        (
            "change",
            "涨跌",
            details
                .change
                .map(|value| format!("{value:+.3}"))
                .unwrap_or_else(|| "—".into()),
            color,
        ),
        (
            "percent",
            "涨幅",
            details
                .percent
                .map(|value| format!("{value:+.2}%"))
                .unwrap_or_else(|| "—".into()),
            color,
        ),
        (
            "average",
            "均价",
            details
                .average
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "—".into()),
            VWAP,
        ),
        (
            "volume",
            "成交量",
            format!("{}股", volume_label(details.volume)),
            BLUE,
        ),
        (
            "turnover",
            "成交额",
            format!(
                "{} {currency}",
                volume_label(details.turnover.round() as u64)
            ),
            BLUE,
        ),
    ];
    let width = 174_f32.min((geometry.width - 8.).max(128.));
    let on_left = intraday_card_on_left(selected_x, geometry);
    let left = if on_left {
        59.
    } else {
        55. + geometry.width - width - 4.
    };
    div()
        .id("chart-intraday-card")
        .test_support()
        .absolute()
        .left(px(left.max(4.)))
        .top(px(28.))
        .w(px(width))
        .max_h(px((geometry.price_height - 8.).max(64.)))
        .overflow_y_scroll()
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(|this, event: &gpui_kit::ClickEvent, _, cx| {
            cx.stop_propagation();
            if let gpui_kit::ClickEvent::Mouse(event) = event
                && (event.up.position - event.down.position).magnitude() > 4.
            {
                return;
            }
            this.clear_inspection();
            cx.notify();
        }))
        .border_1()
        .border_color(rgb(0xCAD8EB))
        .rounded_sm()
        .bg(rgba(0xFFFFFFF0))
        .px_2()
        .py_1()
        .text_size(px(11.))
        .child(
            div()
                .id(if on_left {
                    "chart-intraday-left"
                } else {
                    "chart-intraday-right"
                })
                .test_support()
                .flex()
                .items_center()
                .justify_between()
                .pb_1()
                .mb_1()
                .border_b_1()
                .border_color(rgb(GRID))
                .child(div().text_color(rgb(MUTED)).child("时间"))
                .child(
                    div()
                        .id("chart-intraday-time")
                        .test_support()
                        .aria_label(details.time.clone())
                        .text_color(rgb(0x233348))
                        .child(details.time),
                ),
        )
        .children(fields.into_iter().map(|(key, name, value, color)| {
            div()
                .h(px(18.))
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_color(rgb(MUTED)).child(name))
                .child(
                    div()
                        .id(format!("chart-intraday-{key}"))
                        .test_support()
                        .aria_label(value.clone())
                        .text_color(rgb(color))
                        .child(value),
                )
        }))
        .child(
            div()
                .pt_1()
                .text_size(px(10.))
                .text_color(rgb(MUTED))
                .child("分钟成交"),
        )
}

/// Place the card on the half opposite the inspected candle, keeping the pointer clear.
fn card_on_left(index: usize, count: usize) -> bool {
    index >= count / 2
}

pub(super) fn inspection_card(
    details: &CandleDetails,
    geometry: Geometry,
    index: usize,
    count: usize,
) -> impl IntoElement + use<> {
    let bar = &details.bar;
    let previous = details.previous_close.filter(|price| *price > 0.);
    let value_color = |price: f64| {
        previous.map_or(MUTED, |reference| {
            if (price - reference).abs() < 0.000_001 {
                MUTED
            } else if price > reference {
                UP
            } else {
                DOWN
            }
        })
    };
    let change_color = value_color(bar.close);
    let fields = [
        (
            "open",
            "开盘",
            format!("{:.2}", bar.open),
            value_color(bar.open),
        ),
        (
            "high",
            "最高",
            format!("{:.2}", bar.high),
            value_color(bar.high),
        ),
        (
            "low",
            "最低",
            format!("{:.2}", bar.low),
            value_color(bar.low),
        ),
        ("close", "收盘", format!("{:.2}", bar.close), change_color),
        (
            "change",
            "涨跌",
            previous
                .map(|p| format!("{:+.2}", bar.close - p))
                .unwrap_or_else(|| "—".into()),
            change_color,
        ),
        (
            "percent",
            "涨幅",
            previous
                .map(|p| format!("{:+.2}%", change_percent(bar.close, p)))
                .unwrap_or_else(|| "—".into()),
            change_color,
        ),
        (
            "amplitude",
            "振幅",
            previous
                .map(|p| format!("{:.2}%", (bar.high - bar.low) / p * 100.))
                .unwrap_or_else(|| "—".into()),
            MUTED,
        ),
        (
            "volume",
            "成交量",
            format!("{}股", volume_label(bar.volume)),
            BLUE,
        ),
        (
            "turnover",
            "成交额",
            details
                .turnover
                .map(|value| volume_label(value.round() as u64))
                .unwrap_or_else(|| "—".into()),
            BLUE,
        ),
        (
            "turnover-rate",
            "换手率",
            details
                .turnover_rate
                .map(|value| format!("{value:.2}%"))
                .unwrap_or_else(|| "—".into()),
            MUTED,
        ),
    ];
    let date_label = format!("{}  {}", details.date, details.weekday);
    let width = 174_f32.min((geometry.width - 8.).max(128.));
    let on_left = card_on_left(index, count);
    // Coordinates here are local to the plot container; Geometry uses window coordinates.
    let left = if on_left {
        59.
    } else {
        55. + geometry.width - width - 4.
    };
    div()
        .id("chart-inspection-card")
        .test_support()
        .absolute()
        .left(px(left.max(4.)))
        .top(px(28.))
        .w(px(width))
        .max_h(px((geometry.price_height - 8.).max(64.)))
        .overflow_y_scroll()
        // Scrolling/reading the floating details must not zoom or click the plot beneath it.
        .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .border_1()
        .border_color(rgb(0xCAD8EB))
        .rounded_sm()
        .bg(rgba(0xFFFFFFF0))
        .px_2()
        .py_1()
        .text_size(px(11.))
        .child(
            div()
                .id(if on_left {
                    "chart-inspection-left"
                } else {
                    "chart-inspection-right"
                })
                .test_support()
                .child(
                    div()
                        .id("chart-inspection-date")
                        .test_support()
                        .aria_label(date_label.clone())
                        .pb_1()
                        .mb_1()
                        .border_b_1()
                        .border_color(rgb(GRID))
                        .text_color(rgb(0x233348))
                        .child(date_label),
                ),
        )
        .children(fields.into_iter().map(|(key, name, value, color)| {
            div()
                .h(px(18.))
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_color(rgb(MUTED)).child(name))
                .child(
                    div()
                        .id(format!("chart-inspection-{key}"))
                        .test_support()
                        .aria_label(value.clone())
                        .text_color(rgb(color))
                        .child(value),
                )
        }))
}

pub(super) fn volume_label(volume: u64) -> String {
    if volume >= 100_000_000 {
        format!("{:.2}亿", volume as f64 / 100_000_000.)
    } else if volume >= 10_000 {
        format!("{:.2}万", volume as f64 / 10_000.)
    } else {
        volume.to_string()
    }
}
