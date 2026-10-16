use super::*;

/// Inspect the rendered daily candle, then open its own frozen intraday
/// window. These interactions must not become workspace layout or symbol edits.
pub(super) fn verify_chart_inspection(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let original = cx.update(|cx| workspace::snapshot_board_set(cx));
    let result = verify_chart_interactions(cx, output);
    // A failed assertion must not leave a historical window alive over the
    // restored workspace, either: it is transient and has no saved panel ID.
    for handle in historical_windows(cx)? {
        cx.update_window(handle, |_, window, cx| window.click("historical-close", cx))?;
    }
    let restored = cx.update(|cx| workspace::restore_board_set(original, cx));
    cx.run_until_parked();
    result?;
    restored?;
    render_workspace_windows(cx)?;
    println!(
        "PASS: daily candle click inspection, opposite-edge data cards, pointer leave, keyboard navigation and Escape; correct dates after zoom/pan, historical intraday double-click with matching OHLC/volume, adjacent days and independent close"
    );
    Ok(())
}

pub(super) fn verify_chart_interactions(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let original_board = cx.update(|cx| workspace::active_board_id(cx));
    cx.update(|cx| workspace::create_board("行情检视", workspace::BoardPreset::Empty, cx))?;
    let (_, chart) = add_floating_panel(cx, PanelKind::Chart)?;
    let chart_id = chart.model.id;
    let handle = panel_window(cx, chart_id)?;
    let (_, list) = add_floating_panel(cx, PanelKind::Watchlist)?;
    let list_handle = panel_window(cx, list.model.id)?;
    cx.update_window(handle, |_, window, cx| window.click("period-日 K", cx))?;
    render_workspace_windows(cx)?;
    let symbol = current_panel(cx, chart_id)?.model.symbol;

    // Hover alone is not inspection mode; a click activates it. The info card
    // sits opposite the selected candle, and survives leaving the plot.
    cx.update_window(handle, |_, window, cx| {
        window.hover("native-market-chart", cx);
        ensure!(
            window.try_find("chart-inspection-card").is_none(),
            "Hover alone activated daily candle inspection"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    let left_day = click_daily_fraction(cx, handle, chart_id, 0.25)?;
    verify_candle_card(cx, handle, symbol, left_day, "right")?;
    save_chart_screenshot(cx, handle, output, "daily-crosshair-left")?;
    cx.update_window(handle, |_, window, cx| window.hover("period-日 K", cx))?;
    verify_candle_card(cx, handle, symbol, left_day, "right")?;

    // Keyboard navigation follows chronological trading days, including the
    // weekend gaps in the local calendar, without changing the saved viewport.
    let before_keys = current_panel(cx, chart_id)?.content["chart"].clone();
    cx.update_window(handle, |_, window, cx| window.press("right", cx))?;
    verify_candle_card(cx, handle, symbol, left_day + 1, "right")?;
    cx.update_window(handle, |_, window, cx| window.press("left", cx))?;
    verify_candle_card(cx, handle, symbol, left_day, "right")?;
    ensure!(
        current_panel(cx, chart_id)?.content["chart"] == before_keys,
        "Inspecting an adjacent visible candle changed the chart viewport"
    );
    let right_day = click_daily_fraction(cx, handle, chart_id, 0.75)?;
    verify_candle_card(cx, handle, symbol, right_day, "left")?;
    save_chart_screenshot(cx, handle, output, "daily-crosshair-right")?;
    cx.update_window(handle, |_, window, cx| {
        window.press("escape", cx);
        ensure!(
            window.try_find("chart-inspection-card").is_none(),
            "Escape left a daily candle card visible"
        );
        Ok::<_, anyhow::Error>(())
    })??;

    cx.update_window(handle, |_, window, cx| {
        window.scroll("native-market-chart", ScrollDelta::Lines(point(0., 1.)), cx);
    })?;
    let zoomed_day = click_daily_fraction(cx, handle, chart_id, 0.25)?;
    verify_candle_card(cx, handle, symbol, zoomed_day, "right")?;
    let before_drag = current_panel(cx, chart_id)?.content["chart"].clone();
    cx.update_window(handle, |_, window, cx| {
        let bounds = window.find("native-market-chart").bounds();
        let from = bounds.origin + point(px(70.), px(280.));
        window.drag(from, from + point(px(85.), px(0.)), cx);
    })?;
    ensure!(
        current_panel(cx, chart_id)?.content["chart"] != before_drag
            && historical_windows(cx)?.is_empty(),
        "Dragging failed to pan, or opened a historical window"
    );
    let panned_day = click_daily_fraction(cx, handle, chart_id, 0.25)?;
    verify_candle_card(cx, handle, symbol, panned_day, "right")?;
    ensure!(
        panned_day < zoomed_day,
        "A rightward drag did not reveal older daily candles"
    );

    // Double-click targets the center of the currently zoomed/panned slice,
    // not the same numbered candle in the full history, and never resets it.
    let selected_day = expected_daily_index(&current_panel(cx, chart_id)?, 0.5)?;
    let before_history = cx.update(|cx| workspace::snapshot(cx));
    let windows_before = cx.update(|cx| cx.windows().len());
    cx.update_window(handle, |_, window, cx| {
        window.double_click("native-market-chart", cx)
    })?;
    cx.run_until_parked();
    let histories = historical_windows(cx)?;
    ensure!(
        histories.len() == 1 && cx.update(|cx| cx.windows().len()) == windows_before + 1,
        "Double-clicking a daily candle did not create one independent native window"
    );
    let mut history = histories[0];
    verify_historical_date(cx, history, symbol, selected_day)?;
    ensure!(
        same_board_workspace(&before_history, &cx.update(|cx| workspace::snapshot(cx)))?,
        "Opening historical intraday changed the parent viewport, symbol or workspace"
    );
    save_chart_screenshot(cx, history, output, "historical-intraday")?;
    cx.update_window(handle, |_, window, cx| {
        window.double_click("native-market-chart", cx)
    })?;
    ensure!(
        historical_windows(cx)? == vec![history],
        "Repeated inspection accumulated historical windows for one chart"
    );
    history = verify_historical_intraday_toggle(cx, handle, history, symbol, selected_day)?;

    for (control, day) in [
        ("historical-prev", selected_day - 1),
        ("historical-next", selected_day),
    ] {
        cx.update_window(history, |_, window, cx| window.click(control, cx))?;
        verify_historical_date(cx, history, symbol, day)?;
        ensure_no_intraday_inspection(cx, history)?;
    }
    cx.update_window(history, |_, window, cx| window.press("pageup", cx))?;
    verify_historical_date(cx, history, symbol, selected_day - 1)?;
    cx.update_window(history, |_, window, cx| window.press("pagedown", cx))?;
    verify_historical_date(cx, history, symbol, selected_day)?;
    cx.update_window(history, |_, window, cx| {
        window.click("historical-close", cx)
    })?;
    cx.run_until_parked();
    ensure!(
        cx.update_window(history, |_, _, _| ()).is_err()
            && historical_windows(cx)?.is_empty()
            && same_board_workspace(&before_history, &cx.update(|cx| workspace::snapshot(cx)))?,
        "Closing historical intraday left a window, changed the parent or inserted a component"
    );

    // Period and linked-stock changes clear the old inspection; they must not
    // leave a card showing another instrument or a daily date over intraday.
    click_daily_fraction(cx, handle, chart_id, 0.25)?;
    cx.update_window(handle, |_, window, cx| window.click("period-分时", cx))?;
    render_workspace_windows(cx)?;
    ensure_no_candle_card(cx, handle, "Changing period retained daily inspection")?;
    let intraday_viewport = current_panel(cx, chart_id)?.content["chart"].clone();
    cx.update_window(handle, |_, window, cx| {
        window.double_click("native-market-chart", cx)
    })?;
    ensure!(
        historical_windows(cx)?.is_empty()
            && current_panel(cx, chart_id)?.content["chart"] == intraday_viewport,
        "Double-clicking intraday opened a daily inspector or reset the viewport"
    );
    ensure!(
        toggle_intraday_fraction(cx, handle, 0.2)?.is_some(),
        "The live intraday tab could not open its minute inspector"
    );
    cx.update_window(handle, |_, window, cx| window.click("period-日 K", cx))?;
    render_workspace_windows(cx)?;
    ensure_no_intraday_inspection(cx, handle)?;
    click_daily_fraction(cx, handle, chart_id, 0.25)?;
    cx.update_window(list_handle, |_, window, cx| window.click("symbol-MSFT", cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        current_panel(cx, chart_id)?.model.symbol == Symbol::Msft,
        "The chart did not follow a linked stock selection"
    );
    ensure_no_candle_card(
        cx,
        handle,
        "Changing symbol retained the previous stock's inspection",
    )?;
    cx.update_window(handle, |_, window, cx| {
        window.double_click("native-market-chart", cx)
    })?;
    let temporary = historical_windows(cx)?;
    ensure!(
        temporary.len() == 1,
        "The new stock did not open its own historical session"
    );
    click_board_tab(cx, original_board)?;
    ensure!(
        historical_windows(cx)?.is_empty() && cx.update_window(temporary[0], |_, _, _| ()).is_err(),
        "Switching boards orphaned a historical window from the inactive chart"
    );
    Ok(())
}

pub(super) fn verify_historical_intraday_toggle(
    cx: &mut HeadlessAppContext,
    parent: AnyWindowHandle,
    history: AnyWindowHandle,
    symbol: Symbol,
    day: usize,
) -> Result<AnyWindowHandle> {
    let session = market::historical_session(symbol, day, 0)
        .context("Missing historical intraday fixture")?;
    ensure_no_intraday_inspection(cx, history)?;
    let selected = toggle_intraday_fraction(cx, history, 0.72)?
        .context("The historical minute could not be inspected")?;
    verify_intraday_card(
        cx,
        history,
        &session.bars,
        session.previous_close,
        symbol,
        &selected,
        "left",
    )?;
    cx.update_window(history, |_, window, cx| window.press("escape", cx))?;
    ensure_no_intraday_inspection(cx, history)?;
    verify_historical_date(cx, history, symbol, day)?;
    cx.update_window(history, |_, window, cx| window.press("escape", cx))?;
    cx.run_until_parked();
    ensure!(
        cx.update_window(history, |_, _, _| ()).is_err(),
        "Escape without active inspection did not close the historical window"
    );
    cx.update_window(parent, |_, window, cx| {
        window.double_click("native-market-chart", cx)
    })?;
    let reopened = historical_windows(cx)?;
    ensure!(
        reopened.len() == 1,
        "Reopening historical intraday after Escape did not create one viewer"
    );
    let history = reopened[0];
    ensure_no_intraday_inspection(cx, history)?;
    let selected = toggle_intraday_fraction(cx, history, 0.2)?
        .context("The reopened historical minute could not be inspected")?;
    verify_intraday_card(
        cx,
        history,
        &session.bars,
        session.previous_close,
        symbol,
        &selected,
        "right",
    )?;
    // Leave inspection active: the caller's previous/next-day actions must
    // clear it rather than carrying a minute card across dates.
    Ok(history)
}

pub(super) fn ensure_no_candle_card(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    message: &str,
) -> Result<()> {
    cx.update_window(handle, |_, window, _| {
        ensure!(
            window.try_find("chart-inspection-card").is_none(),
            "{message}"
        );
        Ok(())
    })?
}

pub(super) fn expected_daily_index(panel: &SavedPanel, fraction: f32) -> Result<usize> {
    let viewport: crate::panels::chart::ChartViewport =
        serde_json::from_value(panel.content["chart"].clone())?;
    let length = market::bars(panel.model.symbol, Period::Day, 0).len();
    let count = viewport.bars_visible.min(length);
    let end = length - viewport.offset_from_latest.min(length - count);
    Ok(end - count + (count as f32 * fraction).floor() as usize)
}

pub(super) fn click_daily_fraction(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    panel_id: u64,
    fraction: f32,
) -> Result<usize> {
    let expected = expected_daily_index(&current_panel(cx, panel_id)?, fraction)?;
    cx.update_window(handle, |_, window, cx| {
        let width = window
            .find("native-market-chart")
            .bounds()
            .size
            .width
            .as_f32();
        window.click_at(
            "native-market-chart",
            // Below the card so the click exercises the plot. Reading the
            // card itself deliberately does not reposition its crosshair.
            point(px(55. + (width - 113.) * fraction), px(280.)),
            cx,
        );
    })?;
    Ok(expected)
}

pub(super) fn verify_candle_card(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    symbol: Symbol,
    day: usize,
    side: &str,
) -> Result<()> {
    let session =
        market::historical_session(symbol, day, 0).context("Invalid selected daily index")?;
    cx.update_window(handle, |_, window, _| {
        let card = window.find("chart-inspection-card");
        let plot = window.find("native-market-chart");
        ensure!(
            card.visible() && window.find(format!("chart-inspection-{side}")).visible(),
            "The daily candle card did not dock to the {side} edge"
        );
        ensure!(
            (card.bounds().center().x < plot.bounds().center().x) == (side == "left"),
            "The daily candle card's geometry disagrees with its dock side"
        );
        for (field, expected) in [
            ("date", format!("{}  {}", session.date, session.weekday)),
            ("open", format!("{:.2}", session.daily.open)),
            ("high", format!("{:.2}", session.daily.high)),
            ("low", format!("{:.2}", session.daily.low)),
            ("close", format!("{:.2}", session.daily.close)),
            ("volume", format!("{}股", compact_chart_number(session.daily.volume))),
            ("turnover", compact_chart_number(session.turnover.round() as u64)),
        ] {
            let actual = window.find(format!("chart-inspection-{field}"));
            ensure!(
                actual.label() == Some(expected.as_str()),
                "Daily candle {field} does not match selected day {day}: expected={expected}, actual={:?}",
                actual.label()
            );
        }
        Ok::<_, anyhow::Error>(())
    })??;
    Ok(())
}

pub(super) fn compact_chart_number(value: u64) -> String {
    if value >= 100_000_000 {
        format!("{:.2}亿", value as f64 / 100_000_000.)
    } else if value >= 10_000 {
        format!("{:.2}万", value as f64 / 10_000.)
    } else {
        value.to_string()
    }
}

pub(super) fn historical_windows(cx: &mut HeadlessAppContext) -> Result<Vec<AnyWindowHandle>> {
    cx.run_until_parked();
    let mut result = Vec::new();
    for handle in cx.update(|cx| cx.windows()) {
        if cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.try_find("historical-view").is_some()
        })? {
            result.push(handle);
        }
    }
    Ok(result)
}

pub(super) fn verify_historical_date(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    symbol: Symbol,
    day: usize,
) -> Result<()> {
    let session = market::historical_session(symbol, day, 0).context("Invalid historical day")?;
    let expected_date = format!("{} {}", session.date, session.weekday);
    let expected_symbol = format!("{} {}", symbol.code(), symbol.name());
    let expected_summary = format!(
        "开 {:.2}  高 {:.2}  低 {:.2}  收 {:.2}  量 {} 股  额 {:.2} {}",
        session.daily.open,
        session.daily.high,
        session.daily.low,
        session.daily.close,
        session.daily.volume,
        session.turnover,
        symbol.currency(),
    );
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let date = window.find("historical-date");
        ensure!(
            date.label() == Some(expected_date.as_str())
                && window.find("historical-symbol").label() == Some(expected_symbol.as_str())
                && window.find("native-market-chart").visible(),
            "Historical intraday did not show selected day {}: {:?}",
            session.date,
            date.label()
        );
        let summary = window.find("historical-summary");
        ensure!(
            summary.label() == Some(expected_summary.as_str()),
            "Historical intraday's OHLC, volume or turnover differs from its daily candle: expected={expected_summary}, actual={:?}",
            summary.label()
        );
        Ok::<_, anyhow::Error>(())
    })??;
    Ok(())
}

pub(super) fn save_chart_screenshot(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    output: &Path,
    suffix: &str,
) -> Result<()> {
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("workspace");
    cx.capture_screenshot(handle)
        .context("The chart inspector did not render")?
        .save(output.with_file_name(format!("{stem}-{suffix}.png")))?;
    Ok(())
}
