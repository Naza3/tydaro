//! Native canvas painting; consumes prepared projections and geometry.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_chart(
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
