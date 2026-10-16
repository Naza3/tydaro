use super::*;

/// Prove the native chart consumes injected data, and historical views stay
/// frozen without fetching live mock data during construction or later ticks.
#[cfg(feature = "test-support")]
pub(crate) fn smoke_data_source(cx: &mut gpui_kit::HeadlessAppContext) -> anyhow::Result<()> {
    use crate::adapters::market_data::ChartDataProvider;
    use crate::market::HistoricalSession;
    use anyhow::ensure;
    use gpui_kit::AppContext as _;
    use std::sync::{Arc, Mutex};

    struct Fake(Arc<Mutex<Vec<(bool, u64)>>>);
    impl ChartDataProvider for Fake {
        fn chart(&self, _: Symbol, _: Period, tick: u64) -> ChartData {
            self.0.lock().unwrap().push((false, tick));
            ChartData {
                bars: vec![Bar {
                    time: "2099-01-02".into(),
                    open: 777.,
                    high: 900.,
                    low: 700.,
                    close: 777. + tick as f64,
                    volume: 50,
                }],
                indicator_prefix: Vec::new(),
                reference_price: 700.,
                wall_minute: 600,
                indicative: Vec::new(),
                axis: SessionAxis {
                    start: 500,
                    end: 900,
                    lunch: None,
                    opening_auction: None,
                    closing_auction: None,
                },
                phase: SessionPhase::Morning,
            }
        }
        fn historical(&self, _: Symbol, day_index: usize, tick: u64) -> Option<HistoricalData> {
            self.0.lock().unwrap().push((true, tick));
            let bar = Bar {
                time: "10:00".into(),
                open: 555.,
                high: 600.,
                low: 500.,
                close: 555. + tick as f64,
                volume: 30,
            };
            Some(HistoricalData {
                session: HistoricalSession {
                    day_index,
                    date: "2099-01-02".into(),
                    weekday: "星期五",
                    daily: bar.clone(),
                    previous_close: 500.,
                    bars: vec![bar],
                    indicative: Vec::new(),
                    wall_minute: 600,
                    turnover: 100.,
                    turnover_rate: 1.,
                    previous_day: None,
                    next_day: None,
                },
                axis: SessionAxis {
                    start: 500,
                    end: 900,
                    lunch: None,
                    opening_auction: None,
                    closing_auction: None,
                },
                phase: SessionPhase::Morning,
            })
        }
    }
    let queries = Arc::new(Mutex::new(Vec::new()));
    let source = ChartSource::new(Fake(queries.clone()));
    cx.update(|cx| {
        let chart =
            cx.new(|cx| MarketChart::with_source(Symbol::Aapl, Period::Day, source.clone(), cx));
        ensure!(chart.read(cx).data[0].close == 777.);
        ensure!(chart.read(cx).axis.start == 500);
        chart.update(cx, |chart, cx| {
            chart.set_market(Symbol::Aapl, Period::Day, 9, cx)
        });
        ensure!(chart.read(cx).data[0].close == 786.);
        chart.update(cx, |chart, _| {
            chart.inspection = Some(Inspection {
                time: chart.data[0].time.clone(),
                y_fraction: 0.5,
                on_volume: false,
            });
            chart.refresh_details();
        });
        ensure!(chart.read(cx).details.as_ref().unwrap().date == "2099-01-02");
        chart.update(cx, |chart, _| chart.clear_inspection());
        let frozen = source.historical(Symbol::Aapl, 0, 9).unwrap();
        let history =
            cx.new(|cx| MarketChart::new_historical(Symbol::Aapl, &frozen, source.clone(), cx));
        chart.update(cx, |chart, cx| {
            chart.set_market(Symbol::Aapl, Period::Day, 40, cx)
        });
        history.update(cx, |chart, cx| {
            chart.set_market(Symbol::Aapl, Period::Intraday, 100, cx)
        });
        ensure!(
            history.read(cx).data == frozen.session.bars,
            "History followed the live feed"
        );
        ensure!(
            *queries.lock().unwrap() == [(false, 0), (false, 9), (true, 9), (true, 9), (false, 40)],
            "A frozen chart queried live data"
        );
        Ok(())
    })
}
