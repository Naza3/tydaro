use super::{
    ChartViewport, Indicators, change_percent,
    indicators::{moving_average, session_averages},
    price_extent,
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
    let timeline = super::SessionTimeline::new(market::session_axis(Symbol::Yinli));
    let geometry = super::Geometry::new(gpui_kit::Bounds::new(
        gpui_kit::point(gpui_kit::px(20.), gpui_kit::px(0.)),
        gpui_kit::size(gpui_kit::px(1000.), gpui_kit::px(600.)),
    ));
    let morning = market::bars(Symbol::Yinli, Period::Intraday, 0);
    let latest_morning = morning.last().unwrap();
    assert_eq!(latest_morning.time, "10:00");
    assert!(!super::inspection_card::intraday_card_on_left(
        timeline.x(market::bar_wall_minute(latest_morning), geometry),
        geometry
    ));
    assert!(super::inspection_card::intraday_card_on_left(
        timeline.x(840, geometry),
        geometry
    ));
    // Even after the session fills, the same 10:00 point keeps its card on the right.
    let closed = market::bars(Symbol::Yinli, Period::Intraday, 300);
    let same_minute = closed.iter().find(|bar| bar.time == "10:00").unwrap();
    assert!(!super::inspection_card::intraday_card_on_left(
        timeline.x(market::bar_wall_minute(same_minute), geometry),
        geometry
    ));
}

#[test]
fn fixed_session_coordinates_keep_prices_volumes_and_future_minutes_in_place() {
    let timeline = super::SessionTimeline::new(market::session_axis(Symbol::Yinli));
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
    let timeline = super::SessionTimeline::new(market::session_axis(Symbol::Yinli));
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
    let timeline = super::SessionTimeline::new(market::session_axis(Symbol::Yinli));
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
    let projection =
        crate::adapters::market_data::mock_source().chart(Symbol::Aapl, Period::Minute1, 70);
    let indicators = Indicators::for_projection(&projection);
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
