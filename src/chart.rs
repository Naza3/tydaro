//! Native chart drawing and independent viewports for each chart panel.
use std::{cell::Cell, ops::Range, rc::Rc};

use gpui_kit::{
    App, Bounds, Context, IntoElement, MouseButton, PathBuilder, Pixels, Point, Render,
    ScrollDelta, TestSupportExt, TextAlign, TextRun, Window, canvas, div, fill, point, prelude::*,
    px, rgb, rgba, size,
};
use serde::{Deserialize, Serialize};

use crate::market::{self, Bar, Period, Symbol};

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

    fn pan(&mut self, bars: isize, len: usize) {
        let count = self.range(len).len();
        self.offset_from_latest = self
            .offset_from_latest
            .saturating_add_signed(bars)
            .min(len.saturating_sub(count));
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
    hover: Option<Point<Pixels>>,
    drag: Option<(f32, ChartViewport)>,
}

impl MarketChart {
    pub fn new(symbol: Symbol, period: Period, _cx: &mut Context<Self>) -> Self {
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
            hover: None,
            drag: None,
        }
    }

    pub fn set_market(
        &mut self,
        symbol: Symbol,
        period: Period,
        tick: u64,
        cx: &mut Context<Self>,
    ) {
        if self.symbol == symbol && self.period == period && self.tick == tick {
            return;
        }
        if self.symbol != symbol || self.period != period {
            self.viewport = ChartViewport::default();
            self.hover = None;
        }
        self.symbol = symbol;
        self.period = period;
        self.tick = tick;
        self.data = market::bars(symbol, period, tick);
        self.indicators = Indicators::for_market(symbol, period, tick, &self.data);
        cx.notify();
    }

    pub fn viewport(&self) -> ChartViewport {
        self.viewport
    }

    pub fn restore_viewport(&mut self, viewport: ChartViewport, cx: &mut Context<Self>) {
        self.viewport = viewport;
        cx.notify();
    }

    pub fn zoom_step(&mut self, zoom_in: bool, cx: &mut Context<Self>) {
        self.viewport.zoom(zoom_in, 1., self.data.len());
        cx.notify();
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.viewport = ChartViewport::default();
        self.hover = None;
        cx.notify();
    }
}

impl Render for MarketChart {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let range = self.viewport.range(self.data.len());
        let visible = self.data[range.clone()].to_vec();
        let indicators = self.indicators.slice(range.clone());
        let geometry = Geometry::new(self.bounds.get());
        let selection = self.hover.and_then(|p| geometry.index(p, visible.len()));
        let selected_index = selection.unwrap_or_else(|| visible.len().saturating_sub(1));
        let selected = visible.get(selected_index);
        let intraday = self.period == Period::Intraday;
        let reference_price = market::quote(self.symbol, self.tick).previous_close;
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
            .unwrap_or_default();
        let selected_average = indicators.session.get(selected_index).copied();
        let bounds_cell = self.bounds.clone();
        let hover = self.hover;
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
                    .child(div().flex_shrink_0().text_color(rgb(BLUE)).child(header))
                    .when(intraday, |row| {
                        row.child(
                            div()
                                .flex_shrink_0()
                                .text_color(rgb(VWAP))
                                .child(format!("均价 {:.3}", selected_average.unwrap_or_default())),
                        )
                    }),
            )
            .child(
                div()
                    .id("native-market-chart")
                    .test_support()
                    .relative()
                    .flex_1()
                    .min_h(px(80.))
                    .overflow_hidden()
                    .on_mouse_move(
                        cx.listener(|this, event: &gpui_kit::MouseMoveEvent, _, cx| {
                            if let Some((start_x, viewport)) = this.drag {
                                if event.dragging() {
                                    let geometry = Geometry::new(this.bounds.get());
                                    let count = viewport.range(this.data.len()).len();
                                    let movement = (event.position.x.as_f32() - start_x)
                                        / geometry.width.max(1.);
                                    this.viewport = viewport;
                                    this.viewport.pan(
                                        (movement * count as f32).round() as isize,
                                        this.data.len(),
                                    );
                                } else {
                                    this.drag = None;
                                }
                            }
                            this.hover = Some(event.position);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &gpui_kit::MouseDownEvent, _, cx| {
                            if event.click_count == 2 {
                                this.reset(cx);
                            } else {
                                this.drag = Some((event.position.x.as_f32(), this.viewport));
                            }
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| {
                            this.drag = None;
                        }),
                    )
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        if !hovered {
                            this.hover = None;
                            this.drag = None;
                            cx.notify();
                        }
                    }))
                    .on_scroll_wheel(cx.listener(
                        |this, event: &gpui_kit::ScrollWheelEvent, _, cx| {
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
                                    reference_price,
                                    latest,
                                    hover,
                                    window,
                                    cx,
                                );
                            },
                        )
                        .size_full(),
                    ),
            )
    }
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
    reference_price: f64,
    latest: Option<f64>,
    hover: Option<Point<Pixels>>,
    window: &mut Window,
    cx: &mut App,
) {
    if bars.is_empty() || bounds.size.width < px(150.) || bounds.size.height < px(80.) {
        return;
    }
    let g = Geometry::new(bounds);
    let (minimum, maximum) = price_extent(bars, indicators, intraday, reference_price);
    let y = |price: f64| g.top + ((maximum - price) / (maximum - minimum)) as f32 * g.price_height;
    let selected = hover
        .and_then(|pointer| g.index(pointer, bars.len()))
        .unwrap_or(bars.len() - 1);
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

    // Price and volume share a timeline and vertical grid, as in a trading terminal.
    for i in 0..=4 {
        let fraction = i as f32 / 4.;
        let column = g.left + g.width * fraction;
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
        let index = ((bars.len() - 1) as f32 * fraction).round() as usize;
        let text_x = (column - 18.).clamp(g.left, (g.right() - 43.).max(g.left));
        label(
            bars[index].time.clone(),
            text_x,
            g.bottom + 4.,
            10.,
            rgb(MUTED).into(),
            window,
            cx,
        );
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
        let points: Vec<_> = bars
            .iter()
            .enumerate()
            .map(|(i, b)| (g.x(i, bars.len()), y(b.close)))
            .collect();
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
        let mean_points: Vec<_> = indicators
            .session
            .iter()
            .enumerate()
            .map(|(i, mean)| (g.x(i, bars.len()), y(*mean)))
            .collect();
        stroke(&mean_points, 1.2, VWAP, false, window);
        let bar = &bars[selected];
        label(
            format!("涨幅 {:+.2}%", change_percent(bar.close, reference_price)),
            origin_x + 8.,
            bounds.origin.y.as_f32() + 2.,
            10.,
            rgb(price_color(bar.close)).into(),
            window,
            cx,
        );
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
    let column_width = (g.width / bars.len() as f32 * 0.58).clamp(1., 11.);
    for (i, bar) in bars.iter().enumerate() {
        let top = volume_y(bar.volume as f64);
        rect(
            g.x(i, bars.len()) - column_width / 2.,
            top,
            column_width,
            (g.bottom - top).max(1.),
            if bar.close >= bar.open { UP } else { DOWN },
            window,
        );
    }
    let volume_legend_y = g.price_bottom() + 5.;
    let volume_text = format!("成交量 {}", volume_label(bars[selected].volume));
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

    if let Some(pointer) = hover
        && let Some(index) = g.index(pointer, bars.len())
    {
        let cross_x = g.x(index, bars.len());
        let on_volume = pointer.y.as_f32() >= g.volume_top;
        let cross_y = if on_volume {
            pointer.y.as_f32().clamp(g.volume_top, g.bottom)
        } else {
            pointer.y.as_f32().clamp(g.top, g.price_bottom())
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
        let time_x = (cross_x - 26.).clamp(g.left, (g.right() - 58.).max(g.left));
        rect(time_x, g.bottom + 2., 64., 18., 0x516176, window);
        label(
            bars[index].time.clone(),
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

fn session_averages(bars: &[Bar]) -> Vec<f64> {
    let mut value = 0.;
    let mut volume = 0.;
    bars.iter()
        .map(|bar| {
            let weight = bar.volume as f64;
            // The local feed estimates minute turnover from OHLC in integer
            // cents. Use that same convention so the chart and quote agree.
            let mean_cents =
                ((bar.open + bar.high + bar.low + bar.close) * 100.).round() as u64 / 4;
            value += mean_cents as f64 / 100. * weight;
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
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
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
    fn intraday_average_keeps_session_prefix_when_history_rolls() {
        let bars = market::bars(Symbol::Aapl, Period::Intraday, 70);
        let session = market::session_bars(Symbol::Aapl, 70);
        let indicators = Indicators::for_market(Symbol::Aapl, Period::Intraday, 70, &bars);
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
