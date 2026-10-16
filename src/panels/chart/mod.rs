//! Native chart drawing and independent viewports for each chart panel.
use std::{cell::Cell, ops::Range, rc::Rc};

use gpui_kit::{
    App, Bounds, Context, EventEmitter, FocusHandle, IntoElement, MouseButton, PathBuilder, Pixels,
    Point, Render, ScrollDelta, TestSupportExt, TextAlign, TextRun, Window, canvas, div, fill,
    point, prelude::*, px, rgb, rgba, size,
};

use crate::adapters::market_data::{self, ChartData, ChartSource, HistoricalData};
use crate::market::{self, Bar, IndicativePoint, Period, SessionAxis, SessionPhase, Symbol};

mod geometry;
pub(super) mod history;
mod indicators;
mod inspection_card;
mod interaction;
mod paint;
mod viewport;

use geometry::{Geometry, SessionTimeline, session_segments};
use indicators::{Indicators, bar_turnover, change_percent, price_extent};
use inspection_card::{inspection_card, intraday_inspection_card, volume_label};
use paint::paint_chart;

#[cfg(feature = "test-support")]
mod test_support;
#[cfg(test)]
mod tests;
#[cfg(feature = "test-support")]
pub(crate) use test_support::smoke_data_source;

const BLUE: u32 = 0x397AF6;
const UP: u32 = 0xF0446D;
const DOWN: u32 = 0x00A89B;
const MUTED: u32 = 0x8793A5;
const GRID: u32 = 0xE8EDF3;
const VWAP: u32 = 0xD99935;
const MA_PERIODS: [usize; 5] = [5, 10, 20, 30, 60];
// Keep the workspace's light palette; each moving average has a distinct key.
const MA_COLORS: [u32; 5] = [0x718096, 0xD99935, 0xB260B8, 0x00A89B, BLUE];

pub(crate) use crate::panels::snapshot::ChartViewport;

/// Opening history is separate from changing the live chart's period or viewport.
#[derive(Clone, Debug)]
pub(crate) enum ChartEvent {
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

pub(crate) struct MarketChart {
    source: ChartSource,
    axis: SessionAxis,
    phase: SessionPhase,
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
    pub(crate) fn new(symbol: Symbol, period: Period, cx: &mut Context<Self>) -> Self {
        Self::with_source(symbol, period, market_data::source(cx), cx)
    }

    fn with_source(
        symbol: Symbol,
        period: Period,
        source: ChartSource,
        cx: &mut Context<Self>,
    ) -> Self {
        let projection = source.chart(symbol, period, 0);
        Self::from_projection(symbol, period, source, projection, false, cx)
    }

    fn from_projection(
        symbol: Symbol,
        period: Period,
        source: ChartSource,
        projection: ChartData,
        historical: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let indicators = Indicators::for_projection(&projection);
        Self {
            source,
            axis: projection.axis,
            phase: projection.phase,
            symbol,
            period,
            tick: 0,
            data: projection.bars,
            indicators,
            viewport: ChartViewport::default(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            inspection: None,
            inspecting: false,
            details: None,
            drag: None,
            last_gesture_dragged: false,
            focus: cx.focus_handle(),
            reference_price: projection.reference_price,
            historical,
            wall_minute: projection.wall_minute,
            indicative: projection.indicative,
        }
    }

    pub(crate) fn data_source(&self) -> ChartSource {
        self.source.clone()
    }

    /// A historical inspector consumes its frozen projection directly. It does
    /// not request live data while constructing or following later market ticks.
    pub(crate) fn new_historical(
        symbol: Symbol,
        data: &HistoricalData,
        source: ChartSource,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::from_projection(
            symbol,
            Period::Intraday,
            source,
            data.chart_data(),
            true,
            cx,
        )
    }

    pub(crate) fn set_historical(&mut self, data: &HistoricalData, cx: &mut Context<Self>) {
        self.historical = true;
        self.period = Period::Intraday;
        self.apply_projection(data.chart_data());
        self.viewport = ChartViewport::default();
        self.clear_inspection();
        cx.notify();
    }

    fn apply_projection(&mut self, projection: ChartData) {
        self.indicators = Indicators::for_projection(&projection);
        self.data = projection.bars;
        self.wall_minute = projection.wall_minute;
        self.indicative = projection.indicative;
        self.reference_price = projection.reference_price;
        self.axis = projection.axis;
        self.phase = projection.phase;
    }

    pub(crate) fn set_market(
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
        self.apply_projection(self.source.chart(symbol, period, tick));
        // Re-find the exact time after a rolling update. Never move to a different date.
        if self.inspection.is_some() && self.selected_index().is_none() {
            self.clear_inspection();
        } else {
            self.refresh_details();
        }
        cx.notify();
    }

    pub(crate) fn viewport(&self) -> ChartViewport {
        self.viewport
    }

    pub(crate) fn restore_viewport(&mut self, viewport: ChartViewport, cx: &mut Context<Self>) {
        self.viewport = if self.period == Period::Intraday {
            ChartViewport::default()
        } else {
            viewport
        };
        cx.notify();
    }

    pub(crate) fn zoom_step(&mut self, zoom_in: bool, cx: &mut Context<Self>) {
        if self.period == Period::Intraday {
            return;
        }
        self.viewport.zoom(zoom_in, 1., self.data.len());
        cx.notify();
    }

    pub(crate) fn reset(&mut self, cx: &mut Context<Self>) {
        self.viewport = ChartViewport::default();
        self.clear_inspection();
        cx.notify();
    }
}
