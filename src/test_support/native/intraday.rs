use super::*;

/// Drive the public chart component with the same market ticks used by the
/// workspace clock. No private Session state or fake renderer is involved.
pub(super) fn verify_fixed_intraday(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let (handle, chart) = cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                    point(px(40.), px(40.)),
                    size(px(1040.), px(720.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|cx| MarketChart::new(Symbol::Yinli, Period::Intraday, cx)),
        )
    })?;
    let result = verify_intraday_clock_interactions(cx, handle, &chart, output);
    let closed = cx.update_window(handle, |_, window, _| window.remove_window());
    cx.run_until_parked();
    result?;
    closed?;
    println!(
        "PASS: fixed A-share intraday timeline, separate auction indications, empty future space, morning/lunch/afternoon/closing transitions, preserved opening trades and no post-close wrap; six-stock legacy layouts and A-share text imports"
    );
    Ok(())
}

pub(super) fn verify_intraday_clock_interactions(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    chart: &Entity<MarketChart>,
    output: &Path,
) -> Result<()> {
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    verify_intraday_toggle(cx, handle, chart, output)?;
    verify_intraday_labels(cx, handle, "早盘交易", "10:00", "10:00")?;
    save_chart_screenshot(cx, handle, output, "a-share-morning")?;
    let viewport = cx.update(|cx| chart.read(cx).viewport());
    let morning_time = click_intraday_fraction(cx, handle, 0.15)?
        .context("The early A-share trade is not inspectable")?;
    let early_bar = market::session_bars(Symbol::Yinli, 0)
        .into_iter()
        .find(|bar| bar.time == morning_time)
        .context("The chart selected a nonexistent early trade")?;
    ensure!(
        morning_time.as_str() >= "09:30" && morning_time.as_str() <= "10:00",
        "The initial chart mapped its early region outside the morning session"
    );
    cx.update_window(handle, |_, window, _| {
        ensure!(
            window
                .find("chart-price-header")
                .label()
                .is_some_and(|label| { label.contains(&format!("{:.3}", early_bar.close)) }),
            "The selected intraday header did not show the actual early trade price"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    ensure!(
        click_intraday_fraction(cx, handle, 0.02)?.is_none(),
        "An indicative auction quote was presented as an executed trade"
    );
    ensure!(
        click_intraday_fraction(cx, handle, 0.9)?.is_none(),
        "The morning chart fabricated an afternoon trade in future space"
    );
    click_intraday_fraction(cx, handle, 0.15)?;
    cx.update_window(handle, |_, window, cx| {
        window.scroll("native-market-chart", ScrollDelta::Lines(point(0., 2.)), cx);
        window.scroll("native-market-chart", ScrollDelta::Lines(point(2., 0.)), cx);
        let bounds = window.find("native-market-chart").bounds();
        let from = bounds.origin
            + point(
                px(55. + (bounds.size.width.as_f32() - 113.) * 0.15),
                px(280.),
            );
        window.drag(from, from + point(px(80.), px(0.)), cx);
    })?;
    cx.update(|cx| chart.update(cx, |chart, cx| chart.zoom_step(true, cx)));
    ensure!(
        cx.update(|cx| chart.read(cx).viewport()) == viewport
            && click_intraday_fraction(cx, handle, 0.15)?.as_deref() == Some(morning_time.as_str()),
        "Intraday zoom or drag shifted its fixed market clock"
    );

    for (tick, phase, current_time, last_trade) in [
        (90, "午间休市", "11:30", "11:30"),
        (179, "午间休市", "12:59", "11:30"),
        (180, "午盘交易", "13:00", "13:00"),
        (297, "收盘竞价 · 不可撤单", "14:57", "14:57"),
        (299, "收盘竞价 · 不可撤单", "14:59", "14:57"),
    ] {
        cx.update(|cx| {
            chart.update(cx, |chart, cx| {
                chart.set_market(Symbol::Yinli, Period::Intraday, tick, cx)
            })
        });
        verify_intraday_labels(cx, handle, phase, current_time, last_trade)?;
        ensure!(
            click_intraday_fraction(cx, handle, 0.15)?.as_deref() == Some(morning_time.as_str()),
            "Advancing to {current_time} moved or removed the same morning candle"
        );
        ensure!(
            click_intraday_fraction(cx, handle, 1.)?.is_none(),
            "The chart displayed a closing match before it happened at {current_time}"
        );
    }
    cx.update(|cx| {
        chart.update(cx, |chart, cx| {
            chart.set_market(Symbol::Yinli, Period::Intraday, 300, cx)
        })
    });
    verify_intraday_labels(cx, handle, "已收盘", "15:00", "15:00")?;
    ensure!(
        click_intraday_fraction(cx, handle, 1.)?.as_deref() == Some("15:00"),
        "The completed closing auction has no inspectable executed match"
    );
    cx.update_window(handle, |_, window, cx| window.press("escape", cx))?;
    let closing_header = cx.update_window(handle, |_, window, _| {
        window.find("chart-price-header").label().map(str::to_owned)
    })?;
    save_chart_screenshot(cx, handle, output, "a-share-close")?;
    cx.update(|cx| {
        chart.update(cx, |chart, cx| {
            chart.set_market(Symbol::Yinli, Period::Intraday, 600, cx)
        })
    });
    verify_intraday_labels(cx, handle, "已收盘", "15:00", "15:00")?;
    cx.update_window(handle, |_, window, _| {
        ensure!(
            window.find("chart-price-header").label() == closing_header.as_deref(),
            "Advancing past close restarted or changed the frozen day's latest price"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    ensure!(
        click_intraday_fraction(cx, handle, 0.15)?.as_deref() == Some(morning_time.as_str()),
        "The day's opening history disappeared after the session closed"
    );
    Ok(())
}

pub(super) fn verify_intraday_toggle(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    chart: &Entity<MarketChart>,
    output: &Path,
) -> Result<()> {
    let morning = market::session_bars(Symbol::Yinli, 0);
    let reference = market::quote(Symbol::Yinli, 0).previous_close;
    ensure!(
        move_intraday_fraction(cx, handle, 0.2)?.is_none(),
        "Hover alone opened intraday inspection"
    );
    let selected = toggle_intraday_fraction(cx, handle, 0.2)?
        .context("Clicking the morning trace did not open intraday inspection")?;
    // This sample is near the end of the available morning data, but its
    // fixed clock position is in the left half of the full-day plot.
    let selected_index = morning
        .iter()
        .position(|bar| bar.time == selected)
        .context("The morning card selected a minute outside its session")?;
    ensure!(
        selected_index > morning.len() / 2,
        "The morning fixture no longer distinguishes clock position from data index"
    );
    verify_intraday_card(
        cx,
        handle,
        &morning,
        reference,
        Symbol::Yinli,
        &selected,
        "right",
    )?;

    cx.update_window(handle, |_, window, cx| {
        let plot = window.find("native-market-chart").bounds();
        let from = plot.origin + point(px(55. + (plot.size.width.as_f32() - 113.) * 0.2), px(280.));
        let card = window.find("chart-intraday-card").bounds().center();
        window.drag(from, card, cx);
    })?;
    verify_intraday_card(
        cx,
        handle,
        &morning,
        reference,
        Symbol::Yinli,
        &selected,
        "right",
    )?;
    save_chart_screenshot(cx, handle, output, "intraday-card-right")?;
    cx.update_window(handle, |_, window, cx| {
        window.hover("chart-intraday-card", cx)
    })?;
    verify_intraday_card(
        cx,
        handle,
        &morning,
        reference,
        Symbol::Yinli,
        &selected,
        "right",
    )?;
    cx.update_window(handle, |_, window, cx| window.press("right", cx))?;
    verify_intraday_card(
        cx,
        handle,
        &morning,
        reference,
        Symbol::Yinli,
        &morning[selected_index + 1].time,
        "right",
    )?;
    cx.update_window(handle, |_, window, cx| window.press("left", cx))?;
    verify_intraday_card(
        cx,
        handle,
        &morning,
        reference,
        Symbol::Yinli,
        &selected,
        "right",
    )?;
    let moved = move_intraday_fraction(cx, handle, 0.15)?
        .context("An active intraday crosshair did not follow the pointer")?;
    ensure!(
        moved != selected,
        "Pointer movement kept a stale selected minute"
    );
    verify_intraday_card(
        cx,
        handle,
        &morning,
        reference,
        Symbol::Yinli,
        &moved,
        "right",
    )?;

    ensure!(
        move_intraday_fraction(cx, handle, 0.9)?.is_none(),
        "Moving into future space retained a fake selected minute"
    );
    ensure_no_intraday_inspection(cx, handle)?;
    ensure!(
        move_intraday_fraction(cx, handle, 0.15)?.as_deref() == Some(moved.as_str()),
        "Moving back from blank future space lost the active inspection mode"
    );

    // A second click cancels even when it is on a different valid minute.
    ensure!(
        toggle_intraday_fraction(cx, handle, 0.2)?.is_none(),
        "A second click moved intraday inspection instead of closing it"
    );
    ensure_no_intraday_inspection(cx, handle)?;
    ensure!(
        move_intraday_fraction(cx, handle, 0.15)?.is_none(),
        "Hover resurrected a cancelled intraday crosshair"
    );
    toggle_intraday_fraction(cx, handle, 0.15)?;
    ensure!(
        toggle_intraday_fraction(cx, handle, 0.15)?.is_none(),
        "Clicking the same minute did not toggle inspection off"
    );
    ensure_no_intraday_inspection(cx, handle)?;
    toggle_intraday_fraction(cx, handle, 0.2)?;
    cx.update_window(handle, |_, window, cx| {
        window.click("chart-intraday-card", cx)
    })?;
    ensure_no_intraday_inspection(cx, handle)?;
    toggle_intraday_fraction(cx, handle, 0.2)?;
    cx.update_window(handle, |_, window, cx| window.press("escape", cx))?;
    ensure_no_intraday_inspection(cx, handle)?;
    ensure!(
        move_intraday_fraction(cx, handle, 0.15)?.is_none(),
        "Escape left intraday hover inspection enabled"
    );
    for fraction in [0.02, 0.9] {
        ensure!(
            toggle_intraday_fraction(cx, handle, fraction)?.is_none(),
            "An indicative or future position opened a false trade card"
        );
        ensure_no_intraday_inspection(cx, handle)?;
    }

    cx.update(|cx| {
        chart.update(cx, |chart, cx| {
            chart.set_market(Symbol::Yinli, Period::Intraday, 240, cx)
        })
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    let afternoon = market::session_bars(Symbol::Yinli, 240);
    let selected = toggle_intraday_fraction(cx, handle, 0.72)?
        .context("Clicking the afternoon trace did not open intraday inspection")?;
    verify_intraday_card(
        cx,
        handle,
        &afternoon,
        reference,
        Symbol::Yinli,
        &selected,
        "left",
    )?;
    save_chart_screenshot(cx, handle, output, "intraday-card-left")?;
    let earlier = move_intraday_fraction(cx, handle, 0.2)?
        .context("Moving across the full-day midpoint lost inspection")?;
    verify_intraday_card(
        cx,
        handle,
        &afternoon,
        reference,
        Symbol::Yinli,
        &earlier,
        "right",
    )?;
    let later = move_intraday_fraction(cx, handle, 0.72)?
        .context("Moving back into the afternoon lost inspection")?;
    verify_intraday_card(
        cx,
        handle,
        &afternoon,
        reference,
        Symbol::Yinli,
        &later,
        "left",
    )?;
    // The middle of the explicitly drawn lunch seam has no minute trade.
    let lunch_fraction = cx.update_window(handle, |_, window, _| {
        let width = window
            .find("native-market-chart")
            .bounds()
            .size
            .width
            .as_f32()
            - 113.;
        (100. + 8. + (width - 100. - 16.) / 2. + 4.) / width
    })?;
    ensure!(
        move_intraday_fraction(cx, handle, lunch_fraction)?.is_none(),
        "Moving into the lunch seam retained a false trade card"
    );
    ensure_no_intraday_inspection(cx, handle)?;
    ensure!(
        move_intraday_fraction(cx, handle, 0.72)?.as_deref() == Some(later.as_str()),
        "Moving back from the lunch seam did not restore the active crosshair"
    );
    ensure!(
        toggle_intraday_fraction(cx, handle, lunch_fraction)?.is_none(),
        "The lunch seam was selectable as an executed minute"
    );
    ensure_no_intraday_inspection(cx, handle)?;
    ensure!(
        move_intraday_fraction(cx, handle, 0.72)?.is_none(),
        "Clicking a blank lunch position did not exit inspection mode"
    );

    toggle_intraday_fraction(cx, handle, 0.72)?;
    cx.update(|cx| {
        chart.update(cx, |chart, cx| {
            chart.set_market(Symbol::Yinli, Period::Day, 240, cx)
        })
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    ensure_no_intraday_inspection(cx, handle)?;
    cx.update(|cx| {
        chart.update(cx, |chart, cx| {
            chart.set_market(Symbol::Yinli, Period::Intraday, 240, cx)
        })
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    ensure_no_intraday_inspection(cx, handle)?;
    toggle_intraday_fraction(cx, handle, 0.72)?;
    cx.update(|cx| {
        chart.update(cx, |chart, cx| {
            chart.set_market(Symbol::Yangguang, Period::Intraday, 240, cx)
        })
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    ensure_no_intraday_inspection(cx, handle)?;
    let selected = toggle_intraday_fraction(cx, handle, 0.72)?
        .context("The replacement symbol could not be inspected")?;
    verify_intraday_card(
        cx,
        handle,
        &market::session_bars(Symbol::Yangguang, 240),
        market::quote(Symbol::Yangguang, 240).previous_close,
        Symbol::Yangguang,
        &selected,
        "left",
    )?;
    // Return a fresh morning fixture to the fixed-session boundary checks.
    cx.update(|cx| {
        chart.update(cx, |chart, cx| {
            chart.set_market(Symbol::Yinli, Period::Intraday, 0, cx)
        })
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    ensure_no_intraday_inspection(cx, handle)?;
    println!(
        "PASS: intraday click on/off, moving/keyboard minute inspection, opposite fixed-axis cards with price/change/VWAP/minute volume and turnover; blank-space cancellation and independent period/symbol/history state"
    );
    Ok(())
}

pub(super) fn ensure_no_intraday_inspection(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
) -> Result<()> {
    cx.update_window(handle, |_, window, _| {
        ensure!(
            window.try_find("chart-intraday-card").is_none()
                && window.try_find("chart-selected-time").is_none(),
            "Intraday inspection did not hide both its crosshair and data card"
        );
        Ok(())
    })?
}

pub(super) fn verify_intraday_card(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    bars: &[market::Bar],
    previous_close: f64,
    symbol: Symbol,
    time: &str,
    side: &str,
) -> Result<()> {
    let index = bars
        .iter()
        .position(|bar| bar.time == time)
        .context("The inspection card refers to a nonexistent minute")?;
    let bar = &bars[index];
    let turnover_cents = |bar: &market::Bar| {
        ((bar.open + bar.high + bar.low + bar.close) * 100.).round() as u64 / 4 * bar.volume
    };
    let cumulative_cents: u64 = bars[..=index].iter().map(turnover_cents).sum();
    let cumulative_volume: u64 = bars[..=index].iter().map(|bar| bar.volume).sum();
    let average = cumulative_cents as f64 / 100. / cumulative_volume as f64;
    let minute_turnover = (turnover_cents(bar) as f64 / 100.).round() as u64;
    cx.update_window(handle, |_, window, _| {
        let card = window.find("chart-intraday-card");
        let plot = window.find("native-market-chart");
        ensure!(
            card.visible()
                && window.find(format!("chart-intraday-{side}")).visible()
                && (card.bounds().center().x < plot.bounds().center().x) == (side == "left"),
            "The intraday card did not dock opposite its fixed clock position"
        );
        ensure!(
            window.find("chart-selected-time").label() == Some(time),
            "The intraday crosshair and card disagree on the selected minute"
        );
        for (field, expected) in [
            ("time", time.to_owned()),
            ("latest", format!("{:.3}", bar.close)),
            ("change", format!("{:+.3}", bar.close - previous_close)),
            (
                "percent",
                format!(
                    "{:+.2}%",
                    (bar.close - previous_close) / previous_close * 100.
                ),
            ),
            ("average", format!("{average:.3}")),
            ("volume", format!("{}股", compact_chart_number(bar.volume))),
            (
                "turnover",
                format!(
                    "{} {}",
                    compact_chart_number(minute_turnover),
                    symbol.currency()
                ),
            ),
        ] {
            let actual = window.find(format!("chart-intraday-{field}"));
            ensure!(
                actual.label() == Some(expected.as_str()),
                "Intraday {field} is inconsistent with {time}: expected={expected}, actual={:?}",
                actual.label()
            );
        }
        Ok::<_, anyhow::Error>(())
    })??;
    Ok(())
}

pub(super) fn verify_intraday_labels(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    phase: &str,
    current_time: &str,
    last_trade: &str,
) -> Result<()> {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for (id, expected) in [
            ("intraday-session-phase", phase),
            ("intraday-session-time", current_time),
            ("intraday-last-time", last_trade),
        ] {
            let actual = window.find(id);
            ensure!(
                actual.label() == Some(expected),
                "The session clock did not display {id}={expected}: {:?}",
                actual.label()
            );
        }
        let timeline = window.find("intraday-timeline-label");
        ensure!(
            timeline.label().is_some_and(|label| [
                "09:15",
                "09:20",
                "09:25",
                "09:30",
                "11:30 / 13:00",
                "14:57",
                "15:00"
            ]
            .iter()
            .all(|mark| label.contains(mark))),
            "The A-share intraday timeline lost an auction, lunch or closing boundary"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    Ok(())
}

pub(super) fn click_intraday_fraction(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    fraction: f32,
) -> Result<Option<String>> {
    // Each fixed-clock probe begins outside inspection mode. Otherwise a
    // second-click cancellation could falsely pass an empty-future assertion.
    cx.update_window(handle, |_, window, cx| window.press("escape", cx))?;
    ensure_no_intraday_inspection(cx, handle)?;
    toggle_intraday_fraction(cx, handle, fraction)
}

pub(super) fn toggle_intraday_fraction(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    fraction: f32,
) -> Result<Option<String>> {
    cx.update_window(handle, |_, window, cx| {
        let width = window
            .find("native-market-chart")
            .bounds()
            .size
            .width
            .as_f32();
        window.click_at(
            "native-market-chart",
            point(px(55. + (width - 113.) * fraction), px(280.)),
            cx,
        );
        window
            .try_find("chart-selected-time")
            .and_then(|selected| selected.label().map(str::to_owned))
    })
}

pub(super) fn move_intraday_fraction(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    fraction: f32,
) -> Result<Option<String>> {
    cx.update_window(handle, |_, window, cx| {
        let bounds = window.find("native-market-chart").bounds();
        window.dispatch_event(
            MouseMoveEvent {
                position: bounds.origin
                    + point(
                        px(55. + (bounds.size.width.as_f32() - 113.) * fraction),
                        px(280.),
                    ),
                pressed_button: None,
                modifiers: Default::default(),
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
        window
            .try_find("chart-selected-time")
            .and_then(|selected| selected.label().map(str::to_owned))
    })
}
