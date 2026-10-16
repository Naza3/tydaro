//! Temporary, frozen historical-session viewers owned by their source chart.
use anyhow::{Context as _, Result};
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
    },
    prelude::*,
    *,
};

use crate::{
    chart::MarketChart,
    market::{HistoricalSession, Symbol},
    market_data::ChartSource,
};

#[derive(Clone)]
pub(crate) struct HistoryWindow {
    handle: AnyWindowHandle,
    view: WeakEntity<HistoryView>,
}

impl HistoryWindow {
    /// The source content owns only a weak handle: closing this viewer does not
    /// keep either the chart component or the native window alive.
    pub fn close(self, cx: &mut App) {
        cx.defer(move |cx| {
            let _ = self
                .handle
                .update(cx, |_, window, _| window.remove_window());
        });
    }
}

pub(crate) fn open(
    existing: Option<&HistoryWindow>,
    symbol: Symbol,
    day_index: usize,
    tick: u64,
    data_source: ChartSource,
    source: &Window,
    cx: &mut App,
) -> Result<HistoryWindow> {
    let data = data_source
        .historical(symbol, day_index, tick)
        .context("所选交易日没有可用的分时数据")?;
    let session = data.session.clone();
    if let Some(existing) = existing
        && let Some(view) = existing.view.upgrade()
        && existing
            .handle
            .update(cx, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.symbol = symbol;
                    view.tick = tick;
                    view.source = data_source.clone();
                    // A new source selection can also refer to another symbol.
                    view.chart = cx.new(|cx| {
                        MarketChart::new_historical(symbol, &data, data_source.clone(), cx)
                    });
                    view.session = session.clone();
                    window.focus(&view.focus, cx);
                    cx.notify();
                });
                window.activate_window();
            })
            .is_ok()
    {
        return Ok(existing.clone());
    }

    let display = source.display(cx);
    let mut bounds = Bounds::new(
        source.bounds().origin + point(px(36.), px(36.)),
        size(px(1000.), px(720.)),
    );
    if let Some(display) = &display {
        let visible = display.visible_bounds();
        bounds.size.width = bounds.size.width.min(visible.size.width);
        bounds.size.height = bounds.size.height.min(visible.size.height);
        bounds.origin.x = bounds
            .origin
            .x
            .max(visible.origin.x)
            .min(visible.right() - bounds.size.width);
        bounds.origin.y = bounds
            .origin
            .y
            .max(visible.origin.y)
            .min(visible.bottom() - bounds.size.height);
    }
    let title = format!(
        "{} {} · {} 历史分时 · Tydaro",
        symbol.code(),
        symbol.name(),
        session.date
    );
    let (handle, view) = gpui_kit::open_window(
        WindowOptions {
            display_id: display.map(|display| display.id()),
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(title.into()),
                ..Default::default()
            }),
            window_min_size: Some(size(px(640.), px(420.))),
            ..Default::default()
        },
        cx,
        move |window, cx| {
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                HistoryView {
                    source: data_source.clone(),
                    symbol,
                    tick,
                    chart: cx.new(|cx| {
                        MarketChart::new_historical(symbol, &data, data_source.clone(), cx)
                    }),
                    session,
                    focus,
                }
            })
        },
    )?;
    Ok(HistoryWindow {
        handle,
        view: view.downgrade(),
    })
}

struct HistoryView {
    source: ChartSource,
    symbol: Symbol,
    tick: u64,
    session: HistoricalSession,
    chart: Entity<MarketChart>,
    focus: FocusHandle,
}

impl HistoryView {
    fn navigate(&mut self, next: bool, cx: &mut Context<Self>) {
        let day = if next {
            self.session.next_day
        } else {
            self.session.previous_day
        };
        if let Some(session) =
            day.and_then(|day| self.source.historical(self.symbol, day, self.tick))
        {
            self.chart
                .update(cx, |chart, cx| chart.set_historical(&session, cx));
            self.session = session.session;
            cx.notify();
        }
    }
}

impl Render for HistoryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let date = format!("{} {}", self.session.date, self.session.weekday);
        let symbol = format!("{} {}", self.symbol.code(), self.symbol.name());
        let daily = &self.session.daily;
        let summary = format!(
            "开 {:.2}  高 {:.2}  低 {:.2}  收 {:.2}  量 {} 股  额 {:.2} {}",
            daily.open,
            daily.high,
            daily.low,
            daily.close,
            daily.volume,
            self.session.turnover,
            self.symbol.currency(),
        );
        window.set_window_title(&format!(
            "{symbol} · {} 历史分时 · Tydaro",
            self.session.date
        ));
        div()
            .id("historical-view")
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x233348))
            .text_size(px(12.))
            // Let an active chart inspector consume Escape before closing this viewer.
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "pageup" => this.navigate(false, cx),
                    "pagedown" => this.navigate(true, cx),
                    "escape" => window.remove_window(),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .h(px(38.))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(0xe8ecf0))
                    .child(
                        div()
                            .id("historical-symbol")
                            .aria_label(symbol.clone())
                            .test_support()
                            .font_weight(FontWeight::MEDIUM)
                            .child(symbol),
                    )
                    .child(
                        div()
                            .id("historical-date")
                            .aria_label(date.clone())
                            .test_support()
                            .child(date),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("historical-prev")
                            .label("上一日")
                            .small()
                            .ghost()
                            .tooltip("上一交易日 · PageUp")
                            .disabled(self.session.previous_day.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.navigate(false, cx))),
                    )
                    .child(
                        Button::new("historical-next")
                            .label("下一日")
                            .small()
                            .ghost()
                            .tooltip("下一交易日 · PageDown")
                            .disabled(self.session.next_day.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.navigate(true, cx))),
                    )
                    .child(
                        Button::new("historical-reset")
                            .label("复位")
                            .small()
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.chart.update(cx, |chart, cx| chart.reset(cx))
                            })),
                    )
                    .child(
                        Button::new("historical-close")
                            .label("关闭")
                            .small()
                            .ghost()
                            .on_click(|_, window, _| window.remove_window()),
                    ),
            )
            .child(
                div()
                    .id("historical-summary")
                    .aria_label(summary.clone())
                    .test_support()
                    .px_3()
                    .py_1()
                    .flex_shrink_0()
                    .text_size(px(10.))
                    .text_color(rgb(0x85909f))
                    .child(summary),
            )
            .child(div().flex_1().min_h_0().child(self.chart.clone()))
            .child(
                div()
                    .h(px(26.))
                    .px_3()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(rgb(0xe8ecf0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(10.))
                    .text_color(rgb(0x85909f))
                    .child(format!(
                        "模拟历史分时 · 截至 {} · 已固定数据",
                        self.session
                            .bars
                            .last()
                            .map(|bar| bar.time.as_str())
                            .unwrap_or("—")
                    ))
                    .child("单击切换定位 · PageUp / PageDown 翻日 · Esc 退出"),
            )
    }
}
