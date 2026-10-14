//! Native chart drawing and independent viewports for each chart panel.
use std::{cell::Cell, ops::Range, rc::Rc};

use gpui_kit::{
    App, Bounds, Context, EventEmitter, FocusHandle, IntoElement, MouseButton, PathBuilder, Pixels,
    Point, Render, ScrollDelta, TestSupportExt, TextAlign, TextRun, Window, canvas, div, fill,
    point, prelude::*, px, rgb, rgba, size,
};
use serde::{Deserialize, Serialize};

use crate::market::{self, Bar, HistoricalSession, IndicativePoint, Period, SessionAxis, Symbol};

const BLUE: u32 = 0x397AF6;
const UP: u32 = 0xF0446D;
const DOWN: u32 = 0x00A89B;
const MUTED: u32 = 0x8793A5;
const GRID: u32 = 0xE8EDF3;
const VWAP: u32 = 0xD99935;
const MA_PERIODS: [usize; 5] = [5, 10, 20, 30, 60];
// Keep the workspace's light palette; each moving average has a distinct key.
const MA_COLORS: [u32; 5] = [0x718096, 0xD99935, 0xB260B8, 0x00A89B, BLUE];

/// A viewport is relative to the newest sample and independent for each panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChartViewport {
    pub bars_visible: usize,
    pub offset_from_latest: usize,
}

impl Default for ChartViewport {
    fn default() -> Self {
        Self {
            bars_visible: 240,
            offset_from_latest: 0,
        }
    }
}

impl ChartViewport {
    fn range(self, len: usize) -> Range<usize> {
        if len == 0 {
            return 0..0;
        }
        let count = self.bars_visible.clamp(12.min(len), len);
        let end = len - self.offset_from_latest.min(len - count);
        end - count..end
    }

    fn zoom(&mut self, zoom_in: bool, anchor: f32, len: usize) {
        if len == 0 {
            return;
        }
        let old = self.range(len);
        let old_count = old.len();
        let count = if zoom_in {
            (old_count as f32 * 0.8).round() as usize
        } else {
            (old_count as f32 * 1.25).ceil() as usize
        }
        .clamp(12.min(len), len);
        let anchor = anchor.clamp(0., 1.);
        let fixed = old.start as f32 + anchor * old_count as f32;
        let start = (fixed - anchor * count as f32).round().max(0.) as usize;
        let end = (start + count).min(len);
        self.bars_visible = count;
        self.offset_from_latest = len - end.max(count);
    }

    fn reveal(&mut self, index: usize, len: usize) {
        let range = self.range(len);
        if index < range.start {
            self.offset_from_latest = len.saturating_sub(index + range.len());
        } else if index >= range.end {
            self.offset_from_latest = len.saturating_sub(index + 1);
        }
    }

    fn pan(&mut self, bars: isize, len: usize) {
        let count = self.range(len).len();
        self.offset_from_latest = self
            .offset_from_latest
            .saturating_add_signed(bars)
            .min(len.saturating_sub(count));
    }
}

/// Opening history is separate from changing the live chart's period or viewport.
#[derive(Clone, Debug)]
pub enum ChartEvent {
    InspectDay {
        symbol: Symbol,
        day_index: usize,
        tick: u64,
    },
}

#[derive(Clone)]
struct Inspection {
    // Time identity survives rolling samples; a pixel or retained-array index does not.
    time: String,
    y_fraction: f32,
    on_volume: bool,
}

impl Inspection {
    fn index_in(&self, bars: &[Bar]) -> Option<usize> {
        bars.iter().position(|bar| bar.time == self.time)
    }
}

#[derive(Clone, Copy)]
struct DragState {
    origin: Point<Pixels>,
    viewport: ChartViewport,
    moved: bool,
    click_count: usize,
    inspection_was_active: bool,
}

#[derive(Clone, Copy)]
struct Crosshair {
    index: usize,
    y_fraction: f32,
    on_volume: bool,
}

struct CandleDetails {
    date: String,
    weekday: String,
    bar: Bar,
    previous_close: Option<f64>,
    turnover: Option<f64>,
    turnover_rate: Option<f64>,
}

/// Minute inspection values are independent of the live/latest quote.
struct IntradayDetails {
    time: String,
    latest: f64,
    change: Option<f64>,
    percent: Option<f64>,
    average: Option<f64>,
    volume: u64,
    turnover: f64,
}

impl IntradayDetails {
    fn new(bar: &Bar, previous_close: f64, average: Option<f64>) -> Self {
        let previous = (previous_close > 0.).then_some(previous_close);
        Self {
            time: bar.time.clone(),
            latest: bar.close,
            change: previous.map(|previous| bar.close - previous),
            percent: previous.map(|previous| change_percent(bar.close, previous)),
            average,
            volume: bar.volume,
            turnover: bar_turnover(bar),
        }
    }
}

pub struct MarketChart {
    symbol: Symbol,
    period: Period,
    tick: u64,
    data: Vec<Bar>,
    indicators: Indicators,
    viewport: ChartViewport,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    inspection: Option<Inspection>,
    inspecting: bool,
    details: Option<CandleDetails>,
    drag: Option<DragState>,
    last_gesture_dragged: bool,
    focus: FocusHandle,
    reference_price: f64,
    historical: bool,
    wall_minute: u16,
    indicative: Vec<IndicativePoint>,
}

impl EventEmitter<ChartEvent> for MarketChart {}

impl MarketChart {
    pub fn new(symbol: Symbol, period: Period, cx: &mut Context<Self>) -> Self {
        let data = market::bars(symbol, period, 0);
        let indicators = Indicators::for_market(symbol, period, 0, &data);
        Self {
            symbol,
            period,
            tick: 0,
            data,
            indicators,
            viewport: ChartViewport::default(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            inspection: None,
            inspecting: false,
            details: None,
            drag: None,
            last_gesture_dragged: false,
            focus: cx.focus_handle(),
            reference_price: market::quote(symbol, 0).previous_close,
            historical: false,
            wall_minute: market::session_state(symbol, 0).wall_minute,
            indicative: market::auction_indicative(symbol, 0),
        }
    }

    /// A historical inspector owns its frozen session and never follows live ticks.
    pub fn new_historical(
        symbol: Symbol,
        session: &HistoricalSession,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut chart = Self::new(symbol, Period::Intraday, cx);
        chart.historical = true;
        chart.set_historical(session, cx);
        chart
    }

    pub fn set_historical(&mut self, session: &HistoricalSession, cx: &mut Context<Self>) {
        self.historical = true;
        self.period = Period::Intraday;
        self.data = session.bars.clone();
        self.wall_minute = session.wall_minute;
        self.indicative = session.indicative.clone();
        self.indicators = Indicators::new(&self.data);
        self.reference_price = session.previous_close;
        self.viewport = ChartViewport::default();
        self.clear_inspection();
        cx.notify();
    }

    pub fn set_market(
        &mut self,
        symbol: Symbol,
        period: Period,
        tick: u64,
        cx: &mut Context<Self>,
    ) {
        if self.historical || (self.symbol == symbol && self.period == period && self.tick == tick)
        {
            return;
        }
        if self.symbol != symbol || self.period != period {
            self.viewport = ChartViewport::default();
            self.clear_inspection();
        }
        self.symbol = symbol;
        self.period = period;
        self.tick = tick;
        self.data = market::bars(symbol, period, tick);
        self.wall_minute = market::session_state(symbol, tick).wall_minute;
        self.indicative = market::auction_indicative(symbol, tick);
        self.indicators = Indicators::for_market(symbol, period, tick, &self.data);
        self.reference_price = market::quote(symbol, tick).previous_close;
        // Re-find the exact time after a rolling update. Never move to a different date.
        if self.inspection.is_some() && self.selected_index().is_none() {
            self.clear_inspection();
        } else {
            self.refresh_details();
        }
        cx.notify();
    }

    pub fn viewport(&self) -> ChartViewport {
        self.viewport
    }

    pub fn restore_viewport(&mut self, viewport: ChartViewport, cx: &mut Context<Self>) {
        self.viewport = if self.period == Period::Intraday {
            ChartViewport::default()
        } else {
            viewport
        };
        cx.notify();
    }

    pub fn zoom_step(&mut self, zoom_in: bool, cx: &mut Context<Self>) {
        if self.period == Period::Intraday {
            return;
        }
        self.viewport.zoom(zoom_in, 1., self.data.len());
        cx.notify();
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.viewport = ChartViewport::default();
        self.clear_inspection();
        cx.notify();
    }

    fn clear_inspection(&mut self) {
        self.inspecting = false;
        self.inspection = None;
        self.details = None;
        self.drag = None;
    }

    fn selected_index(&self) -> Option<usize> {
        let selected = self.inspection.as_ref()?;
        selected.index_in(&self.data)
    }

    fn data_range(&self) -> Range<usize> {
        if self.period == Period::Intraday {
            0..self.data.len()
        } else {
            self.viewport.range(self.data.len())
        }
    }

    fn pointer_index(&self, pointer: Point<Pixels>) -> Option<usize> {
        let geometry = Geometry::new(self.bounds.get());
        if self.period == Period::Intraday {
            SessionTimeline::new(self.symbol).index(geometry, pointer, &self.data)
        } else {
            let range = self.data_range();
            geometry
                .index(pointer, range.len())
                .map(|index| range.start + index)
        }
    }

    fn locate(&mut self, pointer: Point<Pixels>) -> Option<usize> {
        let geometry = Geometry::new(self.bounds.get());
        let index = self.pointer_index(pointer)?;
        let on_volume = pointer.y.as_f32() >= geometry.volume_top;
        let (top, height) = if on_volume {
            (geometry.volume_top, geometry.bottom - geometry.volume_top)
        } else {
            (geometry.top, geometry.price_height)
        };
        let changed = self.selected_index() != Some(index);
        self.inspecting = true;
        self.inspection = Some(Inspection {
            time: self.data[index].time.clone(),
            y_fraction: ((pointer.y.as_f32() - top) / height.max(1.)).clamp(0., 1.),
            on_volume,
        });
        if changed {
            self.refresh_details();
        }
        Some(index)
    }

    fn refresh_details(&mut self) {
        self.details = self.selected_index().and_then(|index| {
            if self.period == Period::Intraday {
                return None;
            }
            if self.period == Period::Day {
                let session = market::historical_session(self.symbol, index, self.tick)?;
                Some(CandleDetails {
                    date: session.date,
                    weekday: session.weekday.into(),
                    bar: session.daily,
                    previous_close: Some(session.previous_close),
                    turnover: Some(session.turnover),
                    turnover_rate: Some(session.turnover_rate),
                })
            } else {
                Some(CandleDetails {
                    date: self.data[index].time.clone(),
                    weekday: String::new(),
                    bar: self.data[index].clone(),
                    previous_close: index
                        .checked_sub(1)
                        .map(|previous| self.data[previous].close),
                    turnover: None,
                    turnover_rate: None,
                })
            }
        });
    }

    fn step_inspection(&mut self, right: bool) {
        let Some(index) = self.selected_index() else {
            return;
        };
        let next = if right {
            (index + 1).min(self.data.len() - 1)
        } else {
            index.saturating_sub(1)
        };
        if let Some(inspection) = self.inspection.as_mut() {
            inspection.time = self.data[next].time.clone();
        }
        if self.period != Period::Intraday {
            self.viewport.reveal(next, self.data.len());
        }
        self.refresh_details();
    }
}

impl Render for MarketChart {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let range = self.data_range();
        let visible = self.data[range.clone()].to_vec();
        let indicators = self.indicators.slice(range.clone());
        let geometry = Geometry::new(self.bounds.get());
        let selection = self
            .selected_index()
            .filter(|index| range.contains(index))
            .map(|index| index - range.start);
        let selected_index = selection.unwrap_or_else(|| visible.len().saturating_sub(1));
        let selected = visible.get(selected_index);
        let intraday = self.period == Period::Intraday;
        let reference_price = self.reference_price;
        let header = selected
            .map(|bar| {
                if intraday {
                    format!(
                        "{}  {}  {} {:.3}",
                        self.symbol.name(),
                        bar.time,
                        if selection.is_some() {
                            "价格"
                        } else {
                            "最新"
                        },
                        bar.close,
                    )
                } else {
                    format!(
                        "{}  {}  开 {:.2}  高 {:.2}  低 {:.2}  收 {:.2}",
                        self.symbol.name(),
                        bar.time,
                        bar.open,
                        bar.high,
                        bar.low,
                        bar.close
                    )
                }
            })
            .unwrap_or_else(|| format!("{}  等待成交", self.symbol.name()));
        let phase = market::session_at(self.symbol, self.wall_minute)
            .phase
            .label()
            .to_owned();
        let time_label = market::minute_text(self.wall_minute);
        let latest_time = self
            .data
            .last()
            .map(|bar| bar.time.clone())
            .unwrap_or_else(|| "尚未成交".into());
        let timeline = intraday.then(|| SessionTimeline::new(self.symbol));
        let timeline_label = timeline
            .as_ref()
            .map(SessionTimeline::legend)
            .unwrap_or_default();
        let indicative = self.indicative.clone();
        let selected_time = selection
            .and_then(|index| visible.get(index))
            .map(|bar| bar.time.clone());
        let selected_average = indicators.session.get(selected_index).copied();
        let bounds_cell = self.bounds.clone();
        let crosshair = selection.and_then(|index| {
            self.inspection.as_ref().map(|inspection| Crosshair {
                index,
                y_fraction: inspection.y_fraction,
                on_volume: inspection.on_volume,
            })
        });
        let selected_date = self.selected_index().and_then(|index| {
            (self.period == Period::Day)
                .then(|| market::daily_date(index))
                .flatten()
        });
        let details_card = selection
            .zip(self.details.as_ref())
            .map(|(index, details)| inspection_card(details, geometry, index, visible.len()));
        let intraday_card = selection.and_then(|index| {
            let timeline = timeline.as_ref()?;
            let bar = visible.get(index)?;
            let details =
                IntradayDetails::new(bar, reference_price, indicators.session.get(index).copied());
            let x = timeline.x(market::bar_wall_minute(bar), geometry);
            Some(intraday_inspection_card(
                details,
                geometry,
                x,
                self.symbol.currency(),
                cx,
            ))
        });
        let latest = self.data.last().map(|b| b.close);

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .bg(rgb(0xFFFFFF))
            .child(
                div()
                    .h(px(23.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .overflow_hidden()
                    .child(
                        div()
                            .id("chart-price-header")
                            .test_support()
                            .aria_label(header.clone())
                            .flex_shrink_0()
                            .text_color(rgb(BLUE))
                            .child(header),
                    )
                    .when(intraday, |row| {
                        row.child(
                            div().flex_shrink_0().text_color(rgb(VWAP)).child(
                                selected_average
                                    .map(|value| format!("均价 {value:.3}"))
                                    .unwrap_or_else(|| "均价 —".into()),
                            ),
                        )
                    }),
            )
            .when(intraday, |chart| {
                chart.child(
                    div()
                        .h(px(22.))
                        .flex_shrink_0()
                        .px_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .overflow_hidden()
                        .text_size(px(10.))
                        .text_color(rgb(MUTED))
                        .child(
                            div()
                                .id("intraday-session-phase")
                                .test_support()
                                .aria_label(phase.clone())
                                .flex_shrink_0()
                                .text_color(rgb(BLUE))
                                .child(phase),
                        )
                        .child(
                            div()
                                .id("intraday-session-time")
                                .test_support()
                                .aria_label(time_label.clone())
                                .flex_shrink_0()
                                .child(time_label),
                        )
                        .child(div().flex_shrink_0().child("最近成交"))
                        .child(
                            div()
                                .id("intraday-last-time")
                                .test_support()
                                .aria_label(latest_time.clone())
                                .flex_shrink_0()
                                .child(latest_time),
                        )
                        .child(
                            div()
                                .id("intraday-timeline-label")
                                .test_support()
                                .aria_label(timeline_label.clone())
                                .truncate()
                                .child(timeline_label),
                        ),
                )
            })
            .child(
                div()
                    .id("native-market-chart")
                    .test_support()
                    .track_focus(&self.focus)
                    .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, _, cx| {
                        if !this.inspecting {
                            return;
                        }
                        match event.keystroke.key.as_str() {
                            "escape" => this.clear_inspection(),
                            "left" => this.step_inspection(false),
                            "right" => this.step_inspection(true),
                            _ => return,
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .relative()
                    .flex_1()
                    .min_h(px(80.))
                    .overflow_hidden()
                    .on_mouse_move(
                        cx.listener(|this, event: &gpui_kit::MouseMoveEvent, _, cx| {
                            if let Some(mut drag) = this.drag {
                                if event.dragging() {
                                    let dx = event.position.x.as_f32() - drag.origin.x.as_f32();
                                    let dy = event.position.y.as_f32() - drag.origin.y.as_f32();
                                    drag.moved |= dx.hypot(dy) > 4.;
                                    if drag.moved && this.period != Period::Intraday {
                                        let geometry = Geometry::new(this.bounds.get());
                                        let count = drag.viewport.range(this.data.len()).len();
                                        this.viewport = drag.viewport;
                                        this.viewport.pan(
                                            (dx / geometry.width.max(1.) * count as f32).round()
                                                as isize,
                                            this.data.len(),
                                        );
                                    }
                                    this.drag = Some(drag);
                                    cx.notify();
                                    return;
                                }
                                this.drag = None;
                            }
                            if this.inspecting {
                                if this.locate(event.position).is_none()
                                    && this.period == Period::Intraday
                                {
                                    // Empty future/auction/pause positions are not aliases for the latest trade.
                                    this.inspection = None;
                                    this.details = None;
                                }
                                cx.notify();
                            }
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui_kit::MouseDownEvent, window, cx| {
                            if this.pointer_index(event.position).is_some() {
                                this.focus.focus(window, cx);
                                this.drag = Some(DragState {
                                    origin: event.position,
                                    viewport: this.viewport,
                                    moved: false,
                                    click_count: event.click_count,
                                    inspection_was_active: this.inspecting,
                                });
                            } else if this.period == Period::Intraday {
                                this.clear_inspection();
                                cx.notify();
                            }
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui_kit::MouseUpEvent, _, cx| {
                            let Some(drag) = this.drag.take() else {
                                return;
                            };
                            let was_dragged = this.last_gesture_dragged;
                            this.last_gesture_dragged = drag.moved;
                            if !drag.moved {
                                if this.period == Period::Intraday && drag.inspection_was_active {
                                    // A second click dismisses the whole inspection, even on another minute.
                                    this.clear_inspection();
                                } else if let Some(index) = this.locate(event.position)
                                    && drag.click_count == 2
                                    && !was_dragged
                                    && this.period == Period::Day
                                    && !this.historical
                                {
                                    cx.emit(ChartEvent::InspectDay {
                                        symbol: this.symbol,
                                        day_index: index,
                                        tick: this.tick,
                                    });
                                }
                            }
                            cx.notify();
                        }),
                    )
                    .on_hover(cx.listener(|this, hovered, _, _| {
                        if !hovered {
                            this.drag = None;
                        }
                    }))
                    .on_scroll_wheel(cx.listener(
                        |this, event: &gpui_kit::ScrollWheelEvent, _, cx| {
                            if this.period == Period::Intraday {
                                cx.stop_propagation();
                                return;
                            }
                            let (x, y) = match event.delta {
                                ScrollDelta::Pixels(p) => (p.x.as_f32(), p.y.as_f32()),
                                ScrollDelta::Lines(p) => (p.x * 24., p.y * 24.),
                            };
                            if event.modifiers.shift || x.abs() > y.abs() {
                                let amount = if x.abs() > y.abs() { x } else { y };
                                this.viewport
                                    .pan((amount / 8.).round() as isize, this.data.len());
                            } else if y.abs() > 0.1 {
                                let geometry = Geometry::new(this.bounds.get());
                                let anchor = (event.position.x.as_f32() - geometry.left)
                                    / geometry.width.max(1.);
                                this.viewport.zoom(y > 0., anchor, this.data.len());
                            }
                            cx.stop_propagation();
                            cx.notify();
                        },
                    ))
                    .child(
                        canvas(
                            move |bounds, _, _| bounds_cell.set(bounds),
                            move |bounds, _, window, cx| {
                                paint_chart(
                                    bounds,
                                    &visible,
                                    &indicators,
                                    intraday,
                                    timeline.as_ref(),
                                    &indicative,
                                    reference_price,
                                    latest,
                                    crosshair,
                                    selected_date.as_deref(),
                                    window,
                                    cx,
                                );
                            },
                        )
                        .size_full(),
                    )
                    .children(details_card)
                    .children(intraday_card)
                    .children(selected_time.map(|time| {
                        div()
                            .id("chart-selected-time")
                            .test_support()
                            .aria_label(time.clone())
                            .absolute()
                            .top_0()
                            .right(px(58.))
                            .text_size(px(10.))
                            .text_color(rgb(MUTED))
                            .child(time)
                    })),
            )
    }
}

fn intraday_card_on_left(x: f32, geometry: Geometry) -> bool {
    x >= geometry.left + geometry.width / 2.
}

fn intraday_inspection_card(
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

fn inspection_card(
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

#[derive(Clone, Copy)]
struct Geometry {
    left: f32,
    top: f32,
    width: f32,
    price_height: f32,
    volume_top: f32,
    bottom: f32,
}

impl Geometry {
    fn new(bounds: Bounds<Pixels>) -> Self {
        let height = bounds.size.height.as_f32();
        let top = bounds.origin.y.as_f32() + 24.;
        let price_height = ((height - 62.) * 0.76).max(8.);
        Self {
            left: bounds.origin.x.as_f32() + 55.,
            top,
            width: (bounds.size.width.as_f32() - 113.).max(1.),
            price_height,
            volume_top: top + price_height + 22.,
            bottom: bounds.origin.y.as_f32() + height - 20.,
        }
    }
    fn right(self) -> f32 {
        self.left + self.width
    }
    fn price_bottom(self) -> f32 {
        self.top + self.price_height
    }
    fn x(self, index: usize, count: usize) -> f32 {
        self.left + (index as f32 + 0.5) * self.width / count.max(1) as f32
    }
    fn index(self, pointer: Point<Pixels>, count: usize) -> Option<usize> {
        if count == 0
            || pointer.x.as_f32() < self.left
            || pointer.x.as_f32() > self.right()
            || pointer.y.as_f32() < self.top
            || pointer.y.as_f32() > self.bottom
        {
            return None;
        }
        Some(
            (((pointer.x.as_f32() - self.left) / self.width) * count as f32)
                .floor()
                .max(0.) as usize,
        )
        .map(|i| i.min(count - 1))
    }
}

/// Fixed market-clock coordinates. Empty future minutes and breaks keep their
/// positions even though they have no trade bar yet.
struct SessionTimeline {
    axis: SessionAxis,
}

impl SessionTimeline {
    fn new(symbol: Symbol) -> Self {
        Self {
            axis: market::session_axis(symbol),
        }
    }

    fn cash_start(&self) -> u16 {
        self.axis
            .opening_auction
            .map_or(self.axis.start, |(_, end)| end + 5)
    }

    fn auction_width(&self, geometry: Geometry) -> f32 {
        if self.axis.opening_auction.is_some() {
            (geometry.width * 0.14)
                .clamp(32., 100.)
                .min(geometry.width * 0.28)
        } else {
            0.
        }
    }

    fn opening_gap(&self) -> f32 {
        if self.axis.opening_auction.is_some() {
            8.
        } else {
            0.
        }
    }
    fn lunch_gap(&self) -> f32 {
        if self.axis.lunch.is_some() { 8. } else { 0. }
    }

    fn cash_minutes(&self) -> u16 {
        self.axis.end - self.cash_start() - self.axis.lunch.map_or(0, |(start, end)| end - start)
    }

    fn cash_width(&self, geometry: Geometry) -> f32 {
        (geometry.width - self.auction_width(geometry) - self.opening_gap() - self.lunch_gap())
            .max(1.)
    }

    fn x(&self, minute: u16, geometry: Geometry) -> f32 {
        let auction_width = self.auction_width(geometry);
        if let Some((start, end)) = self.axis.opening_auction {
            if minute <= end {
                return geometry.left
                    + auction_width * minute.saturating_sub(start) as f32
                        / (end - start).max(1) as f32;
            }
            if minute < self.cash_start() {
                return geometry.left
                    + auction_width
                    + self.opening_gap() * (minute - end) as f32
                        / (self.cash_start() - end) as f32;
            }
        }
        let cash_left = geometry.left + auction_width + self.opening_gap();
        let mut elapsed = minute.saturating_sub(self.cash_start());
        let mut break_offset = 0.;
        if let Some((start, end)) = self.axis.lunch {
            if minute >= end {
                elapsed -= end - start;
                break_offset = self.lunch_gap();
            } else if minute > start {
                elapsed = start - self.cash_start();
                break_offset = self.lunch_gap() * (minute - start) as f32 / (end - start) as f32;
            }
        }
        cash_left
            + self.cash_width(geometry) * elapsed as f32 / self.cash_minutes().max(1) as f32
            + break_offset
    }

    fn index(&self, geometry: Geometry, pointer: Point<Pixels>, bars: &[Bar]) -> Option<usize> {
        if pointer.x.as_f32() < geometry.left
            || pointer.x.as_f32() > geometry.right()
            || pointer.y.as_f32() < geometry.top
            || pointer.y.as_f32() > geometry.bottom
        {
            return None;
        }
        let x = pointer.x.as_f32();
        // A matched auction is painted as a 3 px dot. Give that actual dot a
        // finite hit target even when its clock coordinate is between device pixels.
        if let Some((_, end)) = self.axis.opening_auction
            && (x - self.x(end, geometry)).abs() <= 1.5
            && let Some(index) = bars
                .iter()
                .position(|bar| market::bar_wall_minute(bar) == end)
        {
            return Some(index);
        }
        // Exclude wall-clock regions before nearest-point hit testing. In an
        // ultrawide chart a minute's hit radius can be wider than a compressed break.
        if let Some((_, end)) = self.axis.opening_auction {
            let matched = self.x(end, geometry);
            if x < matched || (x > matched && x < self.x(self.cash_start(), geometry)) {
                return None;
            }
        }
        if let Some((start, end)) = self.axis.lunch
            && x > self.x(start, geometry)
            && x < self.x(end, geometry)
        {
            return None;
        }
        if let Some((start, end)) = self.axis.closing_auction
            && x > self.x(start, geometry)
            && x < self.x(end, geometry)
        {
            return None;
        }
        let latest_x = self.x(market::bar_wall_minute(bars.last()?), geometry);
        if x > latest_x + 0.01 {
            return None;
        }
        let tolerance =
            (self.cash_width(geometry) / self.cash_minutes().max(1) as f32 * 0.51).max(0.25);
        bars.iter()
            .enumerate()
            .filter_map(|(index, bar)| {
                let distance =
                    (self.x(market::bar_wall_minute(bar), geometry) - pointer.x.as_f32()).abs();
                (distance <= tolerance).then_some((index, distance))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index)
    }

    fn legend(&self) -> String {
        if self.axis.opening_auction.is_some() {
            "09:15 集合竞价 · 09:20 禁止撤单 · 09:25 撮合 · 09:30 开盘 · 11:30 / 13:00 午休 · 14:57 收盘竞价 · 15:00 收盘".into()
        } else if let Some((start, end)) = self.axis.lunch {
            format!(
                "{} 开盘 · {} / {} 午休 · {} 收盘",
                market::minute_text(self.axis.start),
                market::minute_text(start),
                market::minute_text(end),
                market::minute_text(self.axis.end)
            )
        } else {
            format!(
                "{} 开盘 · {} 收盘 · 固定全天",
                market::minute_text(self.axis.start),
                market::minute_text(self.axis.end)
            )
        }
    }

    fn markers(&self) -> Vec<(u16, String)> {
        if self.axis.opening_auction.is_some() {
            [
                (555, "09:15"),
                (560, "09:20"),
                (565, "09:25"),
                (570, "09:30"),
                (600, "10:00"),
                (630, "10:30"),
                (660, "11:00"),
                (690, "11:30 / 13:00"),
                (810, "13:30"),
                (840, "14:00"),
                (870, "14:30"),
                (900, "15:00"),
            ]
            .into_iter()
            .map(|(minute, text)| (minute, text.into()))
            .collect()
        } else {
            let mut markers = vec![(self.axis.start, market::minute_text(self.axis.start))];
            for minute in (self.axis.start + 30..self.axis.end).step_by(60) {
                if self
                    .axis
                    .lunch
                    .is_none_or(|(start, end)| minute < start || minute > end)
                {
                    markers.push((minute, market::minute_text(minute)));
                }
            }
            if let Some((start, end)) = self.axis.lunch {
                markers.push((
                    start,
                    format!(
                        "{} / {}",
                        market::minute_text(start),
                        market::minute_text(end)
                    ),
                ));
            }
            markers.push((self.axis.end, market::minute_text(self.axis.end)));
            markers.sort_by_key(|(minute, _)| *minute);
            markers
        }
    }
}

fn session_segments(bars: &[Bar]) -> Vec<Range<usize>> {
    let mut segments = Vec::new();
    let mut start = 0;
    for index in 1..bars.len() {
        if market::bar_wall_minute(&bars[index]) != market::bar_wall_minute(&bars[index - 1]) + 1 {
            segments.push(start..index);
            start = index;
        }
    }
    if start < bars.len() {
        segments.push(start..bars.len());
    }
    segments
}

/// Compute indicators against the full history, then slice them with the bars.
/// Panning or zooming must not restart an MA or a session average at the edge.
struct Indicators {
    session: Vec<f64>,
    price: [Vec<Option<f64>>; 5],
    volume: [Vec<Option<f64>>; 2],
}

impl Indicators {
    fn for_market(symbol: Symbol, period: Period, tick: u64, bars: &[Bar]) -> Self {
        if matches!(period, Period::Intraday | Period::Minute1) {
            let session = market::session_bars(symbol, tick);
            let tail = session.len().saturating_sub(bars.len())..session.len();
            Self::new(&session).slice(tail)
        } else {
            Self::new(bars)
        }
    }

    fn new(bars: &[Bar]) -> Self {
        let prices: Vec<_> = bars.iter().map(|bar| bar.close).collect();
        let volumes: Vec<_> = bars.iter().map(|bar| bar.volume as f64).collect();
        Self {
            session: session_averages(bars),
            price: MA_PERIODS.map(|period| moving_average(&prices, period)),
            volume: [5, 10].map(|period| moving_average(&volumes, period)),
        }
    }

    fn slice(&self, range: Range<usize>) -> Self {
        Self {
            session: self.session[range.clone()].to_vec(),
            price: std::array::from_fn(|i| self.price[i][range.clone()].to_vec()),
            volume: std::array::from_fn(|i| self.volume[i][range.clone()].to_vec()),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_chart(
    bounds: Bounds<Pixels>,
    bars: &[Bar],
    indicators: &Indicators,
    intraday: bool,
    timeline: Option<&SessionTimeline>,
    indicative: &[IndicativePoint],
    reference_price: f64,
    latest: Option<f64>,
    crosshair: Option<Crosshair>,
    selected_date: Option<&str>,
    window: &mut Window,
    cx: &mut App,
) {
    if (!intraday && bars.is_empty())
        || bounds.size.width < px(150.)
        || bounds.size.height < px(80.)
    {
        return;
    }
    let g = Geometry::new(bounds);
    let (mut minimum, mut maximum) = price_extent(bars, indicators, intraday, reference_price);
    if intraday {
        let distance = indicative
            .iter()
            .map(|point| (point.price - reference_price).abs() * 1.12)
            .fold((maximum - reference_price).abs(), f64::max);
        minimum = reference_price - distance;
        maximum = reference_price + distance;
    }
    let bar_x = |index: usize| {
        timeline.map_or_else(
            || g.x(index, bars.len()),
            |timeline| timeline.x(market::bar_wall_minute(&bars[index]), g),
        )
    };
    let y = |price: f64| g.top + ((maximum - price) / (maximum - minimum)) as f32 * g.price_height;
    let selected = crosshair
        .map(|crosshair| crosshair.index)
        .unwrap_or(bars.len().saturating_sub(1));
    let origin_x = bounds.origin.x.as_f32();
    let price_color = |price: f64| {
        if (price - reference_price).abs() < 0.000_001 {
            MUTED
        } else if price > reference_price {
            UP
        } else {
            DOWN
        }
    };

    if let Some(timeline) = timeline {
        if let Some((start, end)) = timeline.axis.opening_auction {
            let left = timeline.x(start, g);
            let right = timeline.x(end, g);
            rect(left, g.top, right - left, g.price_height, 0xF5F2FC, window);
            rect(
                left,
                g.volume_top,
                right - left,
                g.bottom - g.volume_top,
                0xF5F2FC,
                window,
            );
            let open = timeline.x(timeline.cash_start(), g);
            rect(
                right,
                g.top,
                open - right,
                g.bottom - g.top,
                0xF3F5F8,
                window,
            );
        }
        if let Some((start, end)) = timeline.axis.lunch {
            let left = timeline.x(start, g);
            let right = timeline.x(end, g);
            rect(
                left,
                g.top,
                right - left,
                g.bottom - g.top,
                0xEEF2F7,
                window,
            );
        }
        if let Some((start, end)) = timeline.axis.closing_auction {
            let left = timeline.x(start, g);
            let right = timeline.x(end, g);
            rect(left, g.top, right - left, g.price_height, 0xF5F2FC, window);
            rect(
                left,
                g.volume_top,
                right - left,
                g.bottom - g.volume_top,
                0xF5F2FC,
                window,
            );
        }
    }

    let row_count = if intraday && g.price_height > 300. {
        8
    } else {
        4
    };
    for i in 0..=row_count {
        let fraction = i as f32 / row_count as f32;
        let row = g.top + g.price_height * fraction;
        stroke(&[(g.left, row), (g.right(), row)], 1., GRID, false, window);
        let price = maximum - (maximum - minimum) * fraction as f64;
        let color = if intraday { price_color(price) } else { MUTED };
        label(
            format!("{price:.2}"),
            origin_x + 5.,
            row - 7.,
            10.,
            rgb(color).into(),
            window,
            cx,
        );
        if intraday {
            label(
                format!("{:+.2}%", change_percent(price, reference_price)),
                g.right() + 5.,
                row - 7.,
                10.,
                rgb(color).into(),
                window,
                cx,
            );
        }
    }

    // Both panes use the same fixed clock coordinates, including blank future time.
    let markers: Vec<_> = if let Some(timeline) = timeline {
        timeline
            .markers()
            .into_iter()
            .map(|(minute, text)| (timeline.x(minute, g), text, Some(minute)))
            .collect()
    } else {
        (0..=4)
            .map(|i| {
                let fraction = i as f32 / 4.;
                let index = ((bars.len() - 1) as f32 * fraction).round() as usize;
                (g.left + g.width * fraction, bars[index].time.clone(), None)
            })
            .collect()
    };
    let is_important = |minute: Option<u16>, text: &str| {
        text.contains('/')
            || minute.is_some_and(|minute| {
                timeline.is_some_and(|timeline| {
                    minute == timeline.cash_start() || minute == timeline.axis.end
                })
            })
    };
    let display_text = |text: &str| {
        if g.width < 260. && text.contains('/') {
            "午休".to_owned()
        } else {
            text.to_owned()
        }
    };
    let text_bounds = |column: f32, text: &str| {
        let width = text.chars().count() as f32 * 5.4;
        let left = (column - width / 2.).clamp(g.left, (g.right() - width).max(g.left));
        (left, left + width)
    };
    let reserved: Vec<_> = markers
        .iter()
        .filter(|(_, text, minute)| is_important(*minute, text))
        .map(|(column, text, _)| text_bounds(*column, &display_text(text)))
        .collect();
    let mut last_label_right = f32::NEG_INFINITY;
    for (column, text, minute) in markers {
        stroke(
            &[(column, g.top), (column, g.price_bottom())],
            1.,
            GRID,
            true,
            window,
        );
        stroke(
            &[(column, g.volume_top), (column, g.bottom)],
            1.,
            GRID,
            true,
            window,
        );
        // The opening match is annotated inside its distinct strip; 09:30 owns the axis below.
        if timeline.is_some_and(|timeline| timeline.axis.opening_auction.is_some())
            && minute == Some(565)
        {
            continue;
        }
        let important = is_important(minute, &text);
        let text = display_text(&text);
        let (text_x, text_right) = text_bounds(column, &text);
        if !important
            && (text_x < last_label_right + 5.
                || reserved
                    .iter()
                    .any(|(left, right)| text_x < *right + 5. && text_right > *left - 5.))
        {
            continue;
        }
        label(
            text,
            text_x,
            g.bottom + 4.,
            10.,
            rgb(MUTED).into(),
            window,
            cx,
        );
        last_label_right = text_right;
    }

    if intraday {
        let baseline = y(reference_price);
        stroke(
            &[(g.left, baseline), (g.right(), baseline)],
            1.2,
            0xC5CFDB,
            false,
            window,
        );
        for segment in session_segments(bars) {
            let points: Vec<_> = segment
                .clone()
                .map(|i| (bar_x(i), y(bars[i].close)))
                .collect();
            if points.len() > 1 {
                let mut area = PathBuilder::fill();
                area.move_to(point(px(points[0].0), px(g.price_bottom())));
                for &(x, y) in &points {
                    area.line_to(point(px(x), px(y)));
                }
                area.line_to(point(px(points[points.len() - 1].0), px(g.price_bottom())));
                area.close();
                if let Ok(path) = area.build() {
                    window.paint_path(path, rgba(0x397AF60B));
                }
                stroke(&points, 1.4, BLUE, false, window);
                let means: Vec<_> = segment
                    .map(|i| (bar_x(i), y(indicators.session[i])))
                    .collect();
                stroke(&means, 1.2, VWAP, false, window);
            } else if let Some(&(x, y)) = points.first() {
                rect(x - 1.5, y - 1.5, 3., 3., BLUE, window);
            }
        }
        if let Some(timeline) = timeline {
            // Indicative auction prices are reference quotes, never matched volume or VWAP input.
            for auction in [timeline.axis.opening_auction, timeline.axis.closing_auction]
                .into_iter()
                .flatten()
            {
                let points: Vec<_> = indicative
                    .iter()
                    .filter(|point| point.wall_minute >= auction.0 && point.wall_minute < auction.1)
                    .map(|point| (timeline.x(point.wall_minute, g), y(point.price)))
                    .collect();
                stroke(&points, 1.1, 0x9270B0, true, window);
            }
            if timeline.axis.opening_auction.is_some() {
                label(
                    "竞价参考".into(),
                    g.left + 3.,
                    g.top + 5.,
                    9.,
                    rgb(0x9270B0).into(),
                    window,
                    cx,
                );
                label(
                    "09:25 撮合".into(),
                    g.left + 3.,
                    g.top + 18.,
                    9.,
                    rgb(MUTED).into(),
                    window,
                    cx,
                );
            }
            if timeline.axis.closing_auction.is_some() {
                label(
                    "14:57 收盘竞价".into(),
                    g.right() - 78.,
                    g.top + 5.,
                    9.,
                    rgb(0x9270B0).into(),
                    window,
                    cx,
                );
            }
        }
        if let Some(bar) = bars.get(selected) {
            label(
                format!("涨幅 {:+.2}%", change_percent(bar.close, reference_price)),
                origin_x + 8.,
                bounds.origin.y.as_f32() + 2.,
                10.,
                rgb(price_color(bar.close)).into(),
                window,
                cx,
            );
        }
        label(
            format!("昨收 {reference_price:.3}"),
            origin_x + 114.,
            bounds.origin.y.as_f32() + 2.,
            10.,
            rgb(MUTED).into(),
            window,
            cx,
        );
    } else {
        let candle_width = (g.width / bars.len() as f32 * 0.62).clamp(1., 11.);
        for (i, bar) in bars.iter().enumerate() {
            let x = g.x(i, bars.len());
            let color = if bar.close >= bar.open { UP } else { DOWN };
            rect(
                x - 0.5,
                y(bar.high),
                1.,
                (y(bar.low) - y(bar.high)).max(1.),
                color,
                window,
            );
            let top = y(bar.open.max(bar.close));
            let height = (y(bar.open.min(bar.close)) - top).max(1.5);
            rect(
                x - candle_width / 2.,
                top,
                candle_width,
                height,
                color,
                window,
            );
        }
        let mut legend_x = origin_x + 8.;
        for (i, series) in indicators.price.iter().enumerate() {
            paint_series(series, g, &y, MA_COLORS[i], window);
            let value = series[selected]
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "—".into());
            let text = format!("MA{}:{value}", MA_PERIODS[i]);
            label(
                text.clone(),
                legend_x,
                bounds.origin.y.as_f32() + 2.,
                10.,
                rgb(MA_COLORS[i]).into(),
                window,
                cx,
            );
            legend_x += text.len() as f32 * 5.7 + 12.;
        }
    }

    let max_volume = indicators
        .volume
        .iter()
        .flat_map(|series| series.iter().flatten())
        .copied()
        .fold(
            bars.iter().map(|bar| bar.volume).max().unwrap_or(1).max(1) as f64,
            f64::max,
        )
        * 1.08;
    let volume_y =
        |volume: f64| g.bottom - (volume / max_volume) as f32 * (g.bottom - g.volume_top).max(1.);
    let volume_rows = if g.bottom - g.volume_top > 65. { 3 } else { 2 };
    for i in 0..=volume_rows {
        let value = max_volume * i as f64 / volume_rows as f64;
        let row = volume_y(value);
        stroke(&[(g.left, row), (g.right(), row)], 1., GRID, false, window);
        label(
            volume_label(value as u64),
            origin_x + 5.,
            row - 7.,
            10.,
            rgb(MUTED).into(),
            window,
            cx,
        );
    }
    let column_width = if let Some(timeline) = timeline {
        (timeline.cash_width(g) / timeline.cash_minutes().max(1) as f32 * 0.58).clamp(1., 11.)
    } else {
        (g.width / bars.len().max(1) as f32 * 0.58).clamp(1., 11.)
    };
    for (i, bar) in bars.iter().enumerate() {
        let top = volume_y(bar.volume as f64);
        rect(
            bar_x(i) - column_width / 2.,
            top,
            column_width,
            (g.bottom - top).max(1.),
            if bar.close >= bar.open { UP } else { DOWN },
            window,
        );
    }
    let volume_legend_y = g.price_bottom() + 5.;
    let volume_text = format!(
        "成交量 {}",
        volume_label(bars.get(selected).map_or(0, |bar| bar.volume))
    );
    label(
        volume_text,
        origin_x + 8.,
        volume_legend_y,
        10.,
        rgb(MUTED).into(),
        window,
        cx,
    );
    if !intraday {
        let mut legend_x = origin_x + 112.;
        for (i, series) in indicators.volume.iter().enumerate() {
            let color = MA_COLORS[i];
            paint_series(series, g, &volume_y, color, window);
            let value = series[selected]
                .map(|value| volume_label(value as u64))
                .unwrap_or_else(|| "—".into());
            let text = format!("MA{}:{value}", [5, 10][i]);
            label(
                text.clone(),
                legend_x,
                volume_legend_y,
                10.,
                rgb(color).into(),
                window,
                cx,
            );
            legend_x += text.len() as f32 * 5.7 + 14.;
        }
    }

    if let Some(price) = latest.filter(|price| *price >= minimum && *price <= maximum) {
        let line_y = y(price);
        stroke(
            &[(g.left, line_y), (g.right(), line_y)],
            1.,
            BLUE,
            true,
            window,
        );
        rect(origin_x + 1., line_y - 8., 51., 17., BLUE, window);
        label(
            format!("{price:.2}"),
            origin_x + 4.,
            line_y - 6.,
            10.,
            rgb(0xFFFFFF).into(),
            window,
            cx,
        );
    }

    if let Some(crosshair) = crosshair {
        let index = crosshair.index;
        let cross_x = bar_x(index);
        let on_volume = crosshair.on_volume;
        let cross_y = if on_volume {
            g.volume_top + crosshair.y_fraction * (g.bottom - g.volume_top)
        } else {
            g.top + crosshair.y_fraction * g.price_height
        };
        stroke(
            &[(cross_x, g.top), (cross_x, g.bottom)],
            1.,
            0x96A3B6,
            true,
            window,
        );
        stroke(
            &[(g.left, cross_y), (g.right(), cross_y)],
            1.,
            0x96A3B6,
            true,
            window,
        );
        let cursor_label = if on_volume {
            let volume =
                ((g.bottom - cross_y) / (g.bottom - g.volume_top).max(1.)) as f64 * max_volume;
            volume_label(volume as u64)
        } else {
            let price = maximum - ((cross_y - g.top) / g.price_height) as f64 * (maximum - minimum);
            if intraday {
                rect(g.right() + 2., cross_y - 8., 55., 17., 0x516176, window);
                label(
                    format!("{:+.2}%", change_percent(price, reference_price)),
                    g.right() + 4.,
                    cross_y - 6.,
                    10.,
                    rgb(0xFFFFFF).into(),
                    window,
                    cx,
                );
            }
            format!("{price:.2}")
        };
        rect(origin_x + 1., cross_y - 8., 51., 17., 0x516176, window);
        label(
            cursor_label,
            origin_x + 4.,
            cross_y - 6.,
            10.,
            rgb(0xFFFFFF).into(),
            window,
            cx,
        );
        let time_text = selected_date.unwrap_or(&bars[index].time);
        let time_width = if selected_date.is_some() { 78. } else { 64. };
        let time_x =
            (cross_x - time_width / 2.).clamp(g.left, (g.right() - time_width).max(g.left));
        rect(time_x, g.bottom + 2., time_width, 18., 0x516176, window);
        label(
            time_text.to_owned(),
            time_x + 5.,
            g.bottom + 3.,
            10.,
            rgb(0xFFFFFF).into(),
            window,
            cx,
        );
    }
}

fn paint_series(
    series: &[Option<f64>],
    geometry: Geometry,
    y: &impl Fn(f64) -> f32,
    color: u32,
    window: &mut Window,
) {
    let points: Vec<_> = series
        .iter()
        .enumerate()
        .filter_map(|(index, value)| value.map(|value| (geometry.x(index, series.len()), y(value))))
        .collect();
    stroke(&points, 1., color, false, window);
}

fn bar_turnover(bar: &Bar) -> f64 {
    let mean_cents = ((bar.open + bar.high + bar.low + bar.close) * 100.).round() as u64 / 4;
    (mean_cents * bar.volume) as f64 / 100.
}

fn session_averages(bars: &[Bar]) -> Vec<f64> {
    let mut value = 0.;
    let mut volume = 0.;
    bars.iter()
        .map(|bar| {
            let weight = bar.volume as f64;
            // The local feed estimates minute turnover from OHLC in integer
            // cents. Use that same convention so the chart and quote agree.
            value += bar_turnover(bar);
            volume += weight;
            if volume > 0. {
                value / volume
            } else {
                bar.close
            }
        })
        .collect()
}

fn moving_average(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut sum = 0.;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            sum += value;
            if index >= period {
                sum -= values[index - period];
            }
            (index + 1 >= period).then_some(sum / period as f64)
        })
        .collect()
}

fn change_percent(price: f64, reference_price: f64) -> f64 {
    if reference_price > 0. && (price - reference_price).abs() > 0.000_000_1 {
        (price - reference_price) / reference_price * 100.
    } else {
        0.
    }
}

fn price_extent(
    bars: &[Bar],
    indicators: &Indicators,
    intraday: bool,
    reference_price: f64,
) -> (f64, f64) {
    let mut minimum = if intraday {
        reference_price
    } else {
        f64::INFINITY
    };
    let mut maximum = if intraday {
        reference_price
    } else {
        f64::NEG_INFINITY
    };
    for bar in bars {
        minimum = minimum.min(if intraday { bar.close } else { bar.low });
        maximum = maximum.max(if intraday { bar.close } else { bar.high });
    }
    if intraday {
        for mean in &indicators.session {
            minimum = minimum.min(*mean);
            maximum = maximum.max(*mean);
        }
        // Equal distances above/below yesterday's close keep price and percent
        // axes aligned, even when the entire session trades on one side.
        let distance = (maximum - reference_price)
            .abs()
            .max((minimum - reference_price).abs())
            * 1.12;
        let distance = distance.max(reference_price.abs() * 0.002).max(0.01);
        return (reference_price - distance, reference_price + distance);
    }
    for mean in indicators
        .price
        .iter()
        .flat_map(|series| series.iter().flatten())
    {
        minimum = minimum.min(*mean);
        maximum = maximum.max(*mean);
    }
    let padding = ((maximum - minimum) * 0.08)
        .max(maximum.abs() * 0.0002)
        .max(0.01);
    (minimum - padding, maximum + padding)
}

fn volume_label(volume: u64) -> String {
    if volume >= 100_000_000 {
        format!("{:.2}亿", volume as f64 / 100_000_000.)
    } else if volume >= 10_000 {
        format!("{:.2}万", volume as f64 / 10_000.)
    } else {
        volume.to_string()
    }
}

fn rect(x: f32, y: f32, width: f32, height: f32, color: u32, window: &mut Window) {
    window.paint_quad(fill(
        Bounds::new(point(px(x), px(y)), size(px(width), px(height))),
        rgb(color),
    ));
}

fn stroke(points: &[(f32, f32)], width: f32, color: u32, dashed: bool, window: &mut Window) {
    let Some(&(x, y)) = points.first() else {
        return;
    };
    let mut builder = PathBuilder::stroke(px(width));
    if dashed {
        builder = builder.dash_array(&[px(3.), px(3.)]);
    }
    builder.move_to(point(px(x), px(y)));
    for &(x, y) in &points[1..] {
        builder.line_to(point(px(x), px(y)));
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, rgb(color));
    }
}

#[allow(clippy::too_many_arguments)]
fn label(
    text: String,
    x: f32,
    y: f32,
    font_size: f32,
    color: gpui_kit::Hsla,
    window: &mut Window,
    cx: &mut App,
) {
    let run = TextRun {
        len: text.len(),
        font: window.text_style().font(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text.into(), px(font_size), &[run], None);
    let _ = line.paint(
        point(px(x), px(y)),
        px(font_size + 4.),
        TextAlign::Left,
        None,
        window,
        cx,
    );
}

#[cfg(test)]
mod tests {
    use super::{
        ChartViewport, Indicators, change_percent, moving_average, price_extent, session_averages,
    };
    use crate::market::{self, Bar, Period, Symbol};

    #[test]
    fn minute_inspection_uses_that_minutes_amount_and_the_vwap_prefix() {
        let bars: Vec<_> = [
            ("09:30", 10., 100),
            ("09:31", 12., 300),
            ("09:32", 99., 9000),
        ]
        .into_iter()
        .map(|(time, close, volume)| Bar {
            time: time.into(),
            open: close,
            high: close,
            low: close,
            close,
            volume,
        })
        .collect();
        let indicators = Indicators::new(&bars);
        let detail = super::IntradayDetails::new(&bars[1], 10., Some(indicators.session[1]));
        assert_eq!(detail.time, "09:31");
        assert_eq!(detail.latest, 12.);
        assert_eq!(detail.change, Some(2.));
        assert_eq!(detail.percent, Some(20.));
        assert_eq!(detail.volume, 300);
        assert_eq!(detail.turnover, 3600.);
        assert_eq!(detail.average, Some(11.5));
        assert_ne!(detail.average, indicators.session.last().copied());
        let fractional = Bar {
            time: "09:33".into(),
            open: 10.01,
            high: 10.03,
            low: 9.99,
            close: 10.02,
            volume: 3,
        };
        assert!((super::bar_turnover(&fractional) - 30.03).abs() < 0.000_001);
        assert!((session_averages(&[fractional])[0] - 10.01).abs() < 0.000_001);
    }

    #[test]
    fn intraday_card_side_uses_the_fixed_clock_instead_of_available_bar_count() {
        let timeline = super::SessionTimeline::new(Symbol::Yinli);
        let geometry = super::Geometry::new(gpui_kit::Bounds::new(
            gpui_kit::point(gpui_kit::px(20.), gpui_kit::px(0.)),
            gpui_kit::size(gpui_kit::px(1000.), gpui_kit::px(600.)),
        ));
        let morning = market::bars(Symbol::Yinli, Period::Intraday, 0);
        let latest_morning = morning.last().unwrap();
        assert_eq!(latest_morning.time, "10:00");
        assert!(!super::intraday_card_on_left(
            timeline.x(market::bar_wall_minute(latest_morning), geometry),
            geometry
        ));
        assert!(super::intraday_card_on_left(
            timeline.x(840, geometry),
            geometry
        ));
        // Even after the session fills, the same 10:00 point keeps its card on the right.
        let closed = market::bars(Symbol::Yinli, Period::Intraday, 300);
        let same_minute = closed.iter().find(|bar| bar.time == "10:00").unwrap();
        assert!(!super::intraday_card_on_left(
            timeline.x(market::bar_wall_minute(same_minute), geometry),
            geometry
        ));
    }

    #[test]
    fn fixed_session_coordinates_keep_prices_volumes_and_future_minutes_in_place() {
        let timeline = super::SessionTimeline::new(Symbol::Yinli);
        let geometry = super::Geometry::new(gpui_kit::Bounds::new(
            gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)),
            gpui_kit::size(gpui_kit::px(1000.), gpui_kit::px(600.)),
        ));
        let morning = market::bars(Symbol::Yinli, Period::Intraday, 0);
        let afternoon = market::bars(Symbol::Yinli, Period::Intraday, 240);
        let pointer = |minute| {
            gpui_kit::point(
                gpui_kit::px(timeline.x(minute, geometry)),
                gpui_kit::px(100.),
            )
        };
        let morning_index = timeline.index(geometry, pointer(600), &morning).unwrap();
        let afternoon_index = timeline.index(geometry, pointer(600), &afternoon).unwrap();
        assert_eq!(morning[morning_index], afternoon[afternoon_index]);
        assert_eq!(morning[morning_index].time, "10:00");
        assert_eq!(timeline.index(geometry, pointer(840), &morning), None);
        assert_eq!(
            afternoon[timeline.index(geometry, pointer(840), &afternoon).unwrap()].time,
            "14:00"
        );
        assert!((timeline.x(900, geometry) - geometry.right()).abs() < 0.001);
        assert!((timeline.x(780, geometry) - timeline.x(690, geometry) - 8.).abs() < 0.001);
    }

    #[test]
    fn paused_and_indicative_regions_never_snap_to_trades_even_on_ultrawide_charts() {
        let timeline = super::SessionTimeline::new(Symbol::Yinli);
        let all = market::bars(Symbol::Yinli, Period::Intraday, 300);
        for width in [240., 1000., 4000., 8000.] {
            let geometry = super::Geometry::new(gpui_kit::Bounds::new(
                gpui_kit::point(gpui_kit::px(0.), gpui_kit::px(0.)),
                gpui_kit::size(gpui_kit::px(width), gpui_kit::px(600.)),
            ));
            for minute in [560, 567, 720, 750, 898, 899] {
                let pointer = gpui_kit::point(
                    gpui_kit::px(timeline.x(minute, geometry)),
                    gpui_kit::px(100.),
                );
                assert_eq!(
                    timeline.index(geometry, pointer, &all),
                    None,
                    "width={width}, minute={minute}"
                );
            }
            for minute in [565, 570, 690, 780, 897, 900] {
                let pointer = gpui_kit::point(
                    gpui_kit::px(timeline.x(minute, geometry)),
                    gpui_kit::px(100.),
                );
                let index = timeline.index(geometry, pointer, &all).unwrap();
                assert_eq!(market::bar_wall_minute(&all[index]), minute);
            }
        }
    }

    #[test]
    fn the_opening_match_has_a_pixel_hit_target_without_making_the_wait_gap_a_trade() {
        let timeline = super::SessionTimeline::new(Symbol::Yinli);
        let bars = market::bars(Symbol::Yinli, Period::Intraday, 0);
        let geometry = super::Geometry::new(gpui_kit::Bounds::new(
            gpui_kit::point(gpui_kit::px(0.4), gpui_kit::px(0.)),
            gpui_kit::size(gpui_kit::px(613.3), gpui_kit::px(600.)),
        ));
        let match_x = timeline.x(565, geometry);
        let integer_pointer = gpui_kit::point(gpui_kit::px(match_x.round()), gpui_kit::px(100.));
        let selected = timeline.index(geometry, integer_pointer, &bars).unwrap();
        assert_eq!(bars[selected].time, "09:25");
        let wait_midpoint = gpui_kit::point(
            gpui_kit::px((match_x + timeline.x(570, geometry)) / 2.),
            gpui_kit::px(100.),
        );
        assert_eq!(timeline.index(geometry, wait_midpoint, &bars), None);
        assert_eq!(timeline.index(geometry, integer_pointer, &[]), None);
    }

    #[test]
    fn session_curve_never_connects_over_auction_or_lunch_gaps() {
        let bars = market::bars(Symbol::Yinli, Period::Intraday, 300);
        let segments = super::session_segments(&bars);
        let endpoints: Vec<_> = segments
            .iter()
            .map(|segment| {
                (
                    bars[segment.start].time.as_str(),
                    bars[segment.end - 1].time.as_str(),
                )
            })
            .collect();
        assert_eq!(
            endpoints,
            vec![
                ("09:25", "09:25"),
                ("09:30", "11:30"),
                ("13:00", "14:57"),
                ("15:00", "15:00")
            ]
        );
        assert_eq!(
            segments.iter().map(|segment| segment.len()).sum::<usize>(),
            bars.len()
        );
    }

    #[test]
    fn inspection_tracks_the_time_identity_when_the_retained_tail_rolls() {
        let initial = market::bars(Symbol::Aapl, Period::Minute1, 30);
        let inspection = super::Inspection {
            time: initial[10].time.clone(),
            y_fraction: 0.5,
            on_volume: false,
        };
        assert_eq!(inspection.index_in(&initial), Some(10));
        let rolled = market::bars(Symbol::Aapl, Period::Minute1, 35);
        assert_eq!(inspection.index_in(&rolled), Some(5));
        // Once that minute leaves retained history, clear rather than showing another bar.
        let expired = market::bars(Symbol::Aapl, Period::Minute1, 50);
        assert_eq!(inspection.index_in(&expired), None);
        let daily = market::bars(Symbol::Aapl, Period::Day, 0);
        let selected_day = super::Inspection {
            time: daily[42].time.clone(),
            y_fraction: 0.5,
            on_volume: false,
        };
        assert_eq!(
            selected_day.index_in(&market::bars(Symbol::Aapl, Period::Day, 70)),
            Some(42)
        );
    }

    #[test]
    fn keyboard_inspection_reveals_each_edge_without_changing_zoom() {
        let mut view = ChartViewport {
            bars_visible: 24,
            offset_from_latest: 30,
        };
        assert_eq!(view.range(180), 126..150);
        view.reveal(125, 180);
        assert_eq!(view.range(180), 125..149);
        view.reveal(179, 180);
        assert_eq!(view.range(180), 156..180);
        view.reveal(0, 180);
        assert_eq!(view.range(180), 0..24);
        assert_eq!(view.bars_visible, 24);
    }

    #[test]
    fn moving_averages_require_history_and_keep_values_after_panning() {
        let values: Vec<_> = (1..=70).map(f64::from).collect();
        let average = moving_average(&values, 60);
        assert!(average[..59].iter().all(Option::is_none));
        assert_eq!(average[59], Some(30.5));
        assert_eq!(average[69], Some(40.5));

        let bars = market::bars(Symbol::Msft, Period::Day, 0);
        let whole = Indicators::new(&bars);
        let visible = whole.slice(120..150);
        for (i, period) in super::MA_PERIODS.into_iter().enumerate() {
            let expected = bars[121 - period..=120]
                .iter()
                .map(|bar| bar.close)
                .sum::<f64>()
                / period as f64;
            assert!((visible.price[i][0].unwrap() - expected).abs() < 0.000_001);
            assert_eq!(visible.price[i], whole.price[i][120..150]);
        }
        assert_eq!(visible.volume[1], whole.volume[1][120..150]);
        assert_eq!(visible.session, whole.session[120..150]);
    }

    #[test]
    fn minute_candle_average_keeps_session_prefix_when_history_rolls() {
        let bars = market::bars(Symbol::Aapl, Period::Minute1, 70);
        let session = market::session_bars(Symbol::Aapl, 70);
        let indicators = Indicators::for_market(Symbol::Aapl, Period::Minute1, 70, &bars);
        let session_means = session_averages(&session);
        let start = session.len() - bars.len();
        assert!(start > 0);
        assert_eq!(indicators.session, session_means[start..]);
        let quote = market::quote(Symbol::Aapl, 70);
        let expected = quote.turnover / quote.volume as f64;
        assert!((indicators.session.last().unwrap() - expected).abs() < 0.000_001);
        // A full MA60 is already available at the front of the retained tail.
        assert!(indicators.price[4][0].is_some());
    }

    #[test]
    fn session_average_is_volume_weighted_and_ignores_zero_volume() {
        let bars: Vec<_> = [(10., 100), (30., 300), (99., 0)]
            .into_iter()
            .map(|(close, volume)| Bar {
                time: String::new(),
                open: close,
                high: close,
                low: close,
                close,
                volume,
            })
            .collect();
        assert_eq!(session_averages(&bars), vec![10., 25., 25.]);
    }

    #[test]
    fn intraday_axes_share_previous_close_even_when_session_is_one_sided() {
        let bars: Vec<_> = [110., 115., 108.]
            .into_iter()
            .map(|close| Bar {
                time: String::new(),
                open: close,
                high: close,
                low: close,
                close,
                volume: 100,
            })
            .collect();
        let indicators = Indicators::new(&bars);
        let (low, high) = price_extent(&bars, &indicators, true, 100.);
        assert!((high - 100. - (100. - low)).abs() < 0.000_001);
        assert!(high > 115. && low < 100.);
        assert_eq!(change_percent(100., 100.), 0.);
        assert!((change_percent(high, 100.) + change_percent(low, 100.)).abs() < 0.000_001);

        let flat = vec![Bar {
            time: String::new(),
            open: 100.,
            high: 100.,
            low: 100.,
            close: 100.,
            volume: 0,
        }];
        let (low, high) = price_extent(&flat, &Indicators::new(&flat), true, 100.);
        assert!(low.is_finite() && high.is_finite() && low < 100. && high > 100.);
    }

    #[test]
    fn corrupt_or_oversized_viewport_stays_inside_data() {
        let view = ChartViewport {
            bars_visible: usize::MAX,
            offset_from_latest: usize::MAX,
        };
        assert_eq!(view.range(120), 0..120);
        assert_eq!(view.range(0), 0..0);
        let tiny = ChartViewport {
            bars_visible: 0,
            offset_from_latest: usize::MAX,
        };
        assert_eq!(tiny.range(120), 0..12);
    }

    #[test]
    fn zoom_keeps_pointer_anchor_and_pan_clamps_to_history() {
        let mut view = ChartViewport::default();
        view.zoom(true, 0.5, 240);
        assert_eq!(view.range(240), 24..216);
        view.pan(isize::MAX, 240);
        assert_eq!(view.range(240), 0..192);
        view.pan(isize::MIN, 240);
        assert_eq!(view.range(240), 48..240);
    }
}
