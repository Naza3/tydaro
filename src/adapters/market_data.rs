//! Application-side chart queries. Providers return already available data;
//! this synchronous view boundary is not a network or broker API.
use std::sync::Arc;

use gpui_kit::{App, Global};

use crate::market::{
    self, Bar, HistoricalSession, IndicativePoint, Period, SessionAxis, SessionPhase, Symbol,
};

#[derive(Clone)]
pub(crate) struct ChartData {
    pub bars: Vec<Bar>,
    /// Earlier bars of the same sampling interval, needed by rolling indicators.
    /// They are not visible and never participate in pointer hit testing.
    pub indicator_prefix: Vec<Bar>,
    pub reference_price: f64,
    pub wall_minute: u16,
    pub indicative: Vec<IndicativePoint>,
    pub axis: SessionAxis,
    pub phase: SessionPhase,
}

#[derive(Clone)]
pub(crate) struct HistoricalData {
    pub session: HistoricalSession,
    pub axis: SessionAxis,
    pub phase: SessionPhase,
}

impl HistoricalData {
    pub fn chart_data(&self) -> ChartData {
        ChartData {
            bars: self.session.bars.clone(),
            indicator_prefix: Vec::new(),
            reference_price: self.session.previous_close,
            wall_minute: self.session.wall_minute,
            indicative: self.session.indicative.clone(),
            axis: self.axis,
            phase: self.phase,
        }
    }
}

/// Exact queries used by the existing chart. `tick` is the current deterministic
/// demo revision, not a claim about a future feed's timestamp protocol.
pub(crate) trait ChartDataProvider: Send + Sync {
    fn chart(&self, symbol: Symbol, period: Period, tick: u64) -> ChartData;
    fn historical(&self, symbol: Symbol, day_index: usize, tick: u64) -> Option<HistoricalData>;
}

#[derive(Clone)]
pub(crate) struct ChartSource(Arc<dyn ChartDataProvider>);

impl ChartSource {
    pub fn new(provider: impl ChartDataProvider + 'static) -> Self {
        Self(Arc::new(provider))
    }
    pub fn chart(&self, symbol: Symbol, period: Period, tick: u64) -> ChartData {
        self.0.chart(symbol, period, tick)
    }
    pub fn historical(
        &self,
        symbol: Symbol,
        day_index: usize,
        tick: u64,
    ) -> Option<HistoricalData> {
        self.0.historical(symbol, day_index, tick)
    }
}

struct MarketData(ChartSource);
impl Global for MarketData {}

pub(crate) fn init_mock(cx: &mut App) {
    cx.set_global(MarketData(mock_source()));
}

pub(crate) fn mock_source() -> ChartSource {
    ChartSource::new(MockChartData)
}

pub(crate) fn source(cx: &App) -> ChartSource {
    cx.global::<MarketData>().0.clone()
}

struct MockChartData;
impl ChartDataProvider for MockChartData {
    fn chart(&self, symbol: Symbol, period: Period, tick: u64) -> ChartData {
        let bars = market::bars(symbol, period, tick);
        let indicator_prefix = if matches!(period, Period::Minute1 | Period::Intraday) {
            let mut history = market::session_bars(symbol, tick);
            history.truncate(history.len().saturating_sub(bars.len()));
            history
        } else {
            Vec::new()
        };
        let state = market::session_state(symbol, tick);
        ChartData {
            bars,
            indicator_prefix,
            reference_price: market::quote(symbol, tick).previous_close,
            wall_minute: state.wall_minute,
            indicative: market::auction_indicative(symbol, tick),
            axis: market::session_axis(symbol),
            phase: state.phase,
        }
    }

    fn historical(&self, symbol: Symbol, day_index: usize, tick: u64) -> Option<HistoricalData> {
        let session = market::historical_session(symbol, day_index, tick)?;
        Some(HistoricalData {
            axis: market::session_axis(symbol),
            phase: market::session_at(symbol, session.wall_minute).phase,
            session,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rolling_projection_retains_indicator_prefix_and_frozen_history() {
        let source = ChartSource::new(MockChartData);
        let frame = source.chart(Symbol::Aapl, Period::Minute1, 70);
        assert!(!frame.indicator_prefix.is_empty());
        let combined: Vec<_> = frame
            .indicator_prefix
            .iter()
            .chain(&frame.bars)
            .cloned()
            .collect();
        assert_eq!(combined, market::session_bars(Symbol::Aapl, 70));
        let day = market::bars(Symbol::Aapl, Period::Day, 0).len() - 1;
        let frozen = source.historical(Symbol::Aapl, day, 0).unwrap();
        let old_bars = frozen.session.bars.clone();
        let _later = source.chart(Symbol::Aapl, Period::Intraday, 120);
        assert_eq!(frozen.chart_data().bars, old_bars);
        assert_eq!(
            source.historical(Symbol::Aapl, day, 0).unwrap().session,
            frozen.session
        );
    }
}
