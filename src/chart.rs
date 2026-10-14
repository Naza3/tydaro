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
    viewport: ChartViewport,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    hover: Option<Point<Pixels>>,
    drag: Option<(f32, ChartViewport)>,
}

impl MarketChart {
    pub fn new(symbol: Symbol, period: Period, _cx: &mut Context<Self>) -> Self {
        Self {
            symbol,
            period,
            tick: 0,
            data: market::bars(symbol, period, 0),
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
        cx.notify();
    }

    pub fn viewport(&self) -> ChartViewport {
        self.viewport
    }

    pub fn restore_viewport(&mut self, viewport: ChartViewport, cx: &mut Context<Self>) {
        self.viewport = viewport;
        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.viewport = ChartViewport::default();
        self.hover = None;
        cx.notify();
    }
}

impl Render for MarketChart {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let range = self.viewport.range(self.data.len());
        let visible = self.data[range.clone()].to_vec();
        let averages = session_averages(&self.data)[range.clone()].to_vec();
        let geometry = Geometry::new(self.bounds.get());
        let selection = self.hover.and_then(|p| geometry.index(p, visible.len()));
        let selected = selection
            .and_then(|i| visible.get(i))
            .or_else(|| visible.last());
        let intraday = self.period == Period::Intraday;
        let header = selected
            .map(|bar| {
                if intraday {
                    format!(
                        "{}   价格 {:.3}   成交量 {}",
                        bar.time,
                        bar.close,
                        volume_label(bar.volume)
                    )
                } else {
                    format!(
                        "{}   开 {:.2}   高 {:.2}   低 {:.2}   收 {:.2}",
                        bar.time, bar.open, bar.high, bar.low, bar.close
                    )
                }
            })
            .unwrap_or_default();
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
                    .h(px(29.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .overflow_hidden()
                    .child(div().text_color(rgb(BLUE)).child(header)),
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
                                    bounds, &visible, &averages, intraday, latest, hover, window,
                                    cx,
                                );
                            },
                        )
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .h(px(26.))
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(rgb(0xEEF1F6))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .child("滚轮缩放 · 拖动平移 · 双击复位")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .id("chart-zoom-out")
                                    .test_support()
                                    .cursor_pointer()
                                    .text_color(rgb(0x52647D))
                                    .child("−")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.viewport.zoom(false, 1., this.data.len());
                                        cx.notify();
                                    })),
                            )
                            .child(div().child(format!("{} 根", range.len())))
                            .child(
                                div()
                                    .id("chart-zoom-in")
                                    .test_support()
                                    .cursor_pointer()
                                    .text_color(rgb(0x52647D))
                                    .child("＋")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.viewport.zoom(true, 1., this.data.len());
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .id("chart-reset")
                                    .test_support()
                                    .cursor_pointer()
                                    .text_color(rgb(BLUE))
                                    .child("复位")
                                    .on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
                            ),
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
        let h = bounds.size.height.as_f32();
        Self {
            left: bounds.origin.x.as_f32() + 12.,
            top: bounds.origin.y.as_f32() + 18.,
            width: (bounds.size.width.as_f32() - 82.).max(1.),
            price_height: (h * 0.73 - 24.).max(1.),
            volume_top: bounds.origin.y.as_f32() + h * 0.79,
            bottom: bounds.origin.y.as_f32() + h - 26.,
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

#[allow(clippy::too_many_arguments)]
fn paint_chart(
    bounds: Bounds<Pixels>,
    bars: &[Bar],
    averages: &[f64],
    intraday: bool,
    latest: Option<f64>,
    hover: Option<Point<Pixels>>,
    window: &mut Window,
    cx: &mut App,
) {
    if bars.is_empty() || bounds.size.width < px(100.) || bounds.size.height < px(80.) {
        return;
    }
    let g = Geometry::new(bounds);
    let (minimum, maximum) = price_extent(bars, averages, intraday);
    let y = |price: f64| g.top + ((maximum - price) / (maximum - minimum)) as f32 * g.price_height;
    let muted = rgb(MUTED);

    for i in 0..=4 {
        let fraction = i as f32 / 4.;
        let row = g.top + g.price_height * fraction;
        stroke(
            &[(g.left, row), (g.right(), row)],
            1.,
            0xEEF1F5,
            false,
            window,
        );
        let price = maximum - (maximum - minimum) * fraction as f64;
        label(
            format!("{price:.3}"),
            g.right() + 8.,
            row - 7.,
            10.,
            muted.into(),
            window,
            cx,
        );
        let column = g.left + g.width * fraction;
        stroke(
            &[(column, g.top), (column, g.bottom)],
            1.,
            0xF2F4F7,
            false,
            window,
        );
        let index = ((bars.len() - 1) as f32 * fraction).round() as usize;
        let text_x = (column - 16.).clamp(g.left, (g.right() - 35.).max(g.left));
        label(
            bars[index].time.clone(),
            text_x,
            g.bottom + 7.,
            10.,
            muted.into(),
            window,
            cx,
        );
    }

    if intraday {
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
            window.paint_path(path, rgba(0x397AF610));
        }
        stroke(&points, 1.5, BLUE, false, window);
        let mean_points: Vec<_> = averages
            .iter()
            .enumerate()
            .map(|(i, a)| (g.x(i, bars.len()), y(*a)))
            .collect();
        stroke(&mean_points, 1.25, 0xE5A043, false, window);
        label(
            "分时".into(),
            g.left + 4.,
            g.top - 18.,
            10.,
            rgb(BLUE).into(),
            window,
            cx,
        );
        label(
            "均价".into(),
            g.left + 39.,
            g.top - 18.,
            10.,
            rgb(0xD99935).into(),
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
        // The overlay is a volume-weighted mean for the displayed history.
        let mean_points: Vec<_> = averages
            .iter()
            .enumerate()
            .map(|(i, a)| (g.x(i, bars.len()), y(*a)))
            .collect();
        stroke(&mean_points, 1., 0xE5A043, false, window);
    }

    let max_volume = bars.iter().map(|b| b.volume).max().unwrap_or(1).max(1) as f32;
    let column_width = (g.width / bars.len() as f32 * 0.58).clamp(1., 11.);
    for (i, bar) in bars.iter().enumerate() {
        let height = (bar.volume as f32 / max_volume * (g.bottom - g.volume_top).max(1.)).max(1.);
        rect(
            g.x(i, bars.len()) - column_width / 2.,
            g.bottom - height,
            column_width,
            height,
            if bar.close >= bar.open { UP } else { DOWN },
            window,
        );
    }
    stroke(
        &[(g.left, g.bottom), (g.right(), g.bottom)],
        1.,
        0xE7ECF2,
        false,
        window,
    );
    label(
        "成交量".into(),
        g.left + 2.,
        g.volume_top - 17.,
        10.,
        muted.into(),
        window,
        cx,
    );
    label(
        volume_label(max_volume as u64),
        g.right() + 8.,
        g.volume_top - 7.,
        10.,
        muted.into(),
        window,
        cx,
    );

    if let Some(price) = latest.filter(|p| *p >= minimum && *p <= maximum) {
        let line_y = y(price);
        stroke(
            &[(g.left, line_y), (g.right(), line_y)],
            1.,
            BLUE,
            true,
            window,
        );
        rect(g.right() + 3., line_y - 9., 62., 18., BLUE, window);
        label(
            format!("{price:.3}"),
            g.right() + 7.,
            line_y - 7.,
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
        let cross_y = pointer.y.as_f32().clamp(g.top, g.price_bottom());
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
        let price = maximum - ((cross_y - g.top) / g.price_height) as f64 * (maximum - minimum);
        rect(g.right() + 3., cross_y - 9., 62., 18., 0x516176, window);
        label(
            format!("{price:.3}"),
            g.right() + 7.,
            cross_y - 7.,
            10.,
            rgb(0xFFFFFF).into(),
            window,
            cx,
        );
        let time_x = (cross_x - 26.).clamp(g.left, (g.right() - 58.).max(g.left));
        rect(time_x, g.bottom + 4., 64., 19., 0x516176, window);
        label(
            bars[index].time.clone(),
            time_x + 5.,
            g.bottom + 6.,
            10.,
            rgb(0xFFFFFF).into(),
            window,
            cx,
        );
    }
}

fn session_averages(bars: &[Bar]) -> Vec<f64> {
    let mut value = 0.;
    let mut volume = 0.;
    bars.iter()
        .map(|bar| {
            let weight = bar.volume.max(1) as f64;
            value += bar.close * weight;
            volume += weight;
            value / volume
        })
        .collect()
}

fn price_extent(bars: &[Bar], averages: &[f64], intraday: bool) -> (f64, f64) {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for bar in bars {
        minimum = minimum.min(if intraday { bar.close } else { bar.low });
        maximum = maximum.max(if intraday { bar.close } else { bar.high });
    }
    for mean in averages {
        minimum = minimum.min(*mean);
        maximum = maximum.max(*mean);
    }
    let padding = ((maximum - minimum) * 0.12)
        .max(maximum.abs() * 0.0002)
        .max(0.01);
    (minimum - padding, maximum + padding)
}

fn volume_label(volume: u64) -> String {
    if volume >= 1_000_000 {
        format!("{:.2}M", volume as f64 / 1_000_000.)
    } else if volume >= 1_000 {
        format!("{:.1}K", volume as f64 / 1_000.)
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
    use super::ChartViewport;

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
