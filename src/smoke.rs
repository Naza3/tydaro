//! Native, offline UI smoke validation, enabled only by `test-support`.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, ClipboardItem, Entity, HeadlessAppContext,
    InputEvent as _, MouseMoveEvent, ScrollDelta, WindowBounds, WindowOptions,
    component::dock::{PanelInfo, PanelState},
    point, px, size,
    test::TestWindowExt,
};

use crate::{
    chart::MarketChart,
    market::{self, Period, Symbol},
    model::PanelKind,
    storage::{self, SavedPanel, WorkspaceFile},
    transfer::DockSide,
    workspace,
};

/// Render the real default application and exercise the public UI with native
/// pointer events. The screenshot uses the real platform headless renderer;
/// missing GPU/software-renderer support is an error, never a blank fallback.
fn open_headless_workspace() -> Result<(HeadlessAppContext, Entity<workspace::Workspace>)> {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(crate::desktop_ui::AppAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| crate::app::init(cx, false));
    let workspace = cx.update(|cx| workspace::open_workspace(0, None, cx))?;
    let handle = cx.update(|cx| workspace.read(cx).handle);
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    Ok((cx, workspace))
}

/// Native event/state regression for platforms without a headless pixel renderer.
/// This entry never claims screenshot or GPU validation; full smoke remains separate.
pub fn run_architecture(path: &str) -> Result<()> {
    let (mut cx, initial) = open_headless_workspace()?;
    cx.update(|cx| cx.set_reduce_motion(true));
    if std::env::args().any(|arg| arg == "--input-only") {
        return verify_rejected_inputs(&mut cx);
    }
    let handle = cx.update(|cx| initial.read(cx).handle);
    for id in ["symbol-MSFT", "period-日 K"] {
        cx.update_window(handle, |_, window, cx| window.click(id, cx))?;
        render_workspace_windows(&mut cx)?;
    }
    crate::contents::smoke_market_panels(&mut cx)?;
    verify_floating_window_close(&mut cx)?;
    let current = cx
        .update(|cx| workspace::workspace(0, cx))
        .context("Main workspace missing")?;
    crate::desktop_smoke::verify(&mut cx, &current, Path::new(path), false)?;
    verify_rejected_inputs(&mut cx)?;
    println!(
        "PASS: architecture native input/state regression; no screenshots or pixel assertions requested"
    );
    Ok(())
}

fn verify_rejected_inputs(cx: &mut HeadlessAppContext) -> Result<()> {
    let handle = cx.update(|cx| {
        workspace::workspace(0, cx)
            .expect("main workspace")
            .read(cx)
            .handle
    });
    crate::journal_page::smoke_rejected_inputs(cx)?;
    crate::research_page::verify_rejected_edits(cx, handle)?;
    crate::watch_page::verify_rejected_edits(cx, handle)?;
    Ok(())
}

/// Focused iteration of the production desktop pages. Full release validation
/// still uses `run`, which includes legacy workspace and chart regressions.
pub fn run_desktop(path: &str) -> Result<()> {
    let (mut cx, workspace) = open_headless_workspace()?;
    let output = Path::new(path);
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Cannot create screenshot directory {}", parent.display()))?;
    }
    verify_desktop_with_failure_capture(&mut cx, &workspace, output)
}

fn verify_desktop_with_failure_capture(
    cx: &mut HeadlessAppContext,
    workspace: &Entity<workspace::Workspace>,
    output: &Path,
) -> Result<()> {
    let result = crate::desktop_smoke::verify(cx, workspace, output, true);
    if result.is_err() {
        let handle = cx.update(|cx| workspace.read(cx).handle);
        let stem = output
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace");
        let path = output.with_file_name(format!("{stem}-desktop-failure.png"));
        match cx
            .capture_screenshot(handle)
            .and_then(|image| Ok(image.save(&path)?))
        {
            Ok(()) => eprintln!("Desktop failure screenshot: {}", path.display()),
            Err(error) => eprintln!("Could not capture desktop failure: {error:#}"),
        }
        let other_windows = cx.update(|cx| cx.windows());
        for (index, other) in other_windows
            .into_iter()
            .filter(|other| *other != handle)
            .enumerate()
        {
            let path = output.with_file_name(format!("{stem}-desktop-failure-window-{index}.png"));
            match cx
                .capture_screenshot(other)
                .and_then(|image| Ok(image.save(&path)?))
            {
                Ok(()) => eprintln!("Additional native failure screenshot: {}", path.display()),
                Err(error) => eprintln!("Could not capture additional native failure: {error:#}"),
            }
        }
    }
    result
}

pub fn run(path: &str) -> Result<()> {
    let (mut cx, workspace) = open_headless_workspace()?;
    let handle = cx.update(|cx| workspace.read(cx).handle);
    cx.update(workspace::verify_transfer_geometry)?;
    let initial = cx.update(|cx| workspace.read(cx).dock.read(cx).dump(cx));
    let mut initial_panels = Vec::new();
    collect_panels(&initial.center, &mut initial_panels)?;
    for kind in [PanelKind::Watchlist, PanelKind::Chart, PanelKind::Quote] {
        ensure!(
            initial_panels.iter().any(|panel| panel.model.kind == kind),
            "Default workspace is missing the {} panel",
            kind.title()
        );
    }
    ensure!(
        initial_panels
            .iter()
            .filter(|panel| matches!(panel.model.kind, PanelKind::Chart | PanelKind::Quote))
            .all(|panel| panel.model.symbol == Symbol::Yinli)
            && initial_panels
                .iter()
                .filter(|panel| panel.model.kind == PanelKind::Watchlist)
                .all(|panel| list_members(panel).is_ok_and(|members| members == Symbol::ALL)),
        "The fresh workspace did not start with A-share charts and the expanded demo catalog"
    );

    let screenshot = cx
        .capture_screenshot(handle)
        .context("The native headless renderer could not capture the workspace")?;
    ensure!(
        screenshot.width() >= 800 && screenshot.height() >= 500,
        "The rendered workspace is unexpectedly small: {} × {}",
        screenshot.width(),
        screenshot.height()
    );
    let colorful = screenshot
        .pixels()
        .filter(|p| p[0].abs_diff(p[1]) > 24 || p[1].abs_diff(p[2]) > 24)
        .count();
    let dark = screenshot
        .pixels()
        .filter(|p| p[0] < 100 && p[1] < 120 && p[2] < 140)
        .count();
    ensure!(
        colorful > 1_000 && dark > 200,
        "The rendered image lacks expected chart and text pixels"
    );
    let output = Path::new(path);
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Cannot create screenshot directory {}", parent.display()))?;
    }
    screenshot
        .save(output)
        .with_context(|| format!("Cannot save screenshot {}", output.display()))?;

    cx.update_window(handle, |_, window, cx| {
        ensure!(
            window.find("symbol-MSFT").visible(),
            "MSFT is not visible in the default watchlist"
        );
        window.click("symbol-MSFT", cx);
        Ok::<_, anyhow::Error>(())
    })??;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    let changed = cx.update(|cx| workspace.read(cx).dock.read(cx).dump(cx));
    let mut changed_panels = Vec::new();
    collect_panels(&changed.center, &mut changed_panels)?;
    let linked: Vec<_> = changed_panels
        .iter()
        .filter(|panel| panel.model.link_group == 1)
        .collect();
    ensure!(
        linked.len() >= 3,
        "Expected at least three linked market panels"
    );
    ensure!(
        linked
            .iter()
            .all(|panel| panel.model.symbol == Symbol::Msft),
        "Clicking MSFT did not update all panels in link group 1"
    );

    cx.update_window(handle, |_, window, cx| window.click("period-日 K", cx))?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("workspace");
    cx.capture_screenshot(handle)
        .context("The candle chart did not render")?
        .save(output.with_file_name(format!("{stem}-candles.png")))?;
    let changed = cx.update(|cx| workspace.read(cx).dock.read(cx).dump(cx));
    changed_panels.clear();
    collect_panels(&changed.center, &mut changed_panels)?;
    ensure!(
        changed_panels.iter().any(|panel| {
            panel.model.kind == PanelKind::Chart
                && panel.model.symbol == Symbol::Msft
                && panel.model.period == Period::Day
        }),
        "Clicking 日 K did not change the selected chart period"
    );
    cx.update_window(handle, |_, window, cx| {
        window.scroll("native-market-chart", ScrollDelta::Lines(point(0., 1.)), cx);
    })?;
    let zoomed = cx.update(|cx| workspace.read(cx).dock.read(cx).dump(cx));
    let mut zoomed_panels = Vec::new();
    collect_panels(&zoomed.center, &mut zoomed_panels)?;
    let chart = zoomed_panels
        .iter()
        .find(|panel| panel.model.kind == PanelKind::Chart)
        .context("The chart disappeared after scrolling")?;
    let visible = chart
        .content
        .get("chart")
        .and_then(|v| v.get("bars_visible"))
        .and_then(serde_json::Value::as_u64)
        .context("The chart viewport was not persisted")?;
    ensure!(
        visible > 0 && visible < market::bars(Symbol::Msft, Period::Day, 0).len() as u64,
        "A native scroll did not zoom the chart viewport"
    );
    // Persistence must include live UI changes, not merely the initial model.
    let encoded = serde_json::to_vec(&zoomed)?;
    let decoded: gpui_kit::component::dock::DockAreaState = serde_json::from_slice(&encoded)?;
    ensure!(
        decoded == zoomed,
        "The live Dock state did not survive JSON serialization"
    );
    verify_detach_roundtrip(&mut cx, chart)?;
    verify_new_floating_panels(&mut cx, output)?;
    crate::contents::smoke_demo_orders(&mut cx)?;
    crate::contents::smoke_market_panels(&mut cx)?;
    verify_boards(&mut cx, output)?;
    verify_floating_window_close(&mut cx)?;
    verify_stock_lists(&mut cx, output)?;
    verify_chart_inspection(&mut cx, output)?;
    verify_stock_import(&mut cx, output)?;
    verify_fixed_intraday(&mut cx, output)?;
    let current = cx
        .update(|cx| workspace::workspace(0, cx))
        .context("Main workspace missing before desktop checks")?;
    verify_desktop_with_failure_capture(&mut cx, &current, output)?;
    verify_rejected_inputs(&mut cx)?;

    println!(
        "PASS: native workspace {} × {}, {} panels; MSFT link group, 日 K, chart zoom; detach/return state, persistence/undo and demo order interactions. Screenshot: {}",
        screenshot.width(),
        screenshot.height(),
        initial_panels.len(),
        output.display()
    );
    Ok(())
}

/// Drive the public chart component with the same market ticks used by the
/// workspace clock. No private Session state or fake renderer is involved.
fn verify_fixed_intraday(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
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

fn verify_intraday_clock_interactions(
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

fn verify_intraday_toggle(
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

fn ensure_no_intraday_inspection(
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

fn verify_intraday_card(
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

fn verify_intraday_labels(
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

fn click_intraday_fraction(
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

fn toggle_intraday_fraction(
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

fn move_intraday_fraction(
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

/// Previewing pasted text must be read-only until confirmation. Imported
/// membership belongs to one component, even when its quote is linked.
fn verify_stock_import(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let original = cx.update(|cx| workspace::snapshot_board_set(cx));
    let result = verify_stock_import_interactions(cx, output);
    let restored = cx.update(|cx| workspace::restore_board_set(original, cx));
    cx.run_until_parked();
    result?;
    restored?;
    render_workspace_windows(cx)?;
    println!(
        "PASS: multiline clipboard stock import, read-only preview/cancel, duplicates/existing/unknown feedback, append-only confirmation, stale-preview rejection, group isolation and imported-membership persistence"
    );
    Ok(())
}

fn verify_stock_import_interactions(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let original_board = cx.update(|cx| workspace::active_board_id(cx));
    let board =
        cx.update(|cx| workspace::create_board("文本导入", workspace::BoardPreset::Empty, cx))?;
    let (owner, target) = add_floating_panel(cx, PanelKind::Watchlist)?;
    let (_, peer) = add_floating_panel(cx, PanelKind::Watchlist)?;
    let (_, chart) = add_floating_panel(cx, PanelKind::Chart)?;
    let id = target.model.id;
    rename_panel(cx, id, "批量观察")?;
    for symbol in Symbol::ALL
        .into_iter()
        .filter(|symbol| *symbol != Symbol::Tencent)
    {
        click_list_control(cx, id, &format!("stock-list-remove-{}", symbol.code()))?;
    }
    set_list_input(cx, id, "watchlist-search", "腾讯")?;
    click_list_control(cx, id, "watchlist-sort")?;
    let before = current_panel(cx, id)?;
    let peer_before = current_panel(cx, peer.model.id)?;
    let chart_before = current_panel(cx, chart.model.id)?;

    click_list_control(cx, id, "stock-list-import")?;
    ensure!(
        stock_import_value(cx, id)?.is_empty(),
        "A new import opened with an old draft"
    );
    click_list_control(cx, id, "stock-import-confirm")?;
    paste_stock_import(cx, id, "TSLA")?;
    expect_import_summary(cx, id, "可添加 1 支 · 已有 0 支 · 重复 0 次 · 未收录 0 项")?;
    click_list_control(cx, id, "stock-import-cancel")?;
    ensure!(
        current_panel(cx, id)?.content == before.content,
        "Cancelling a text preview or confirming empty text changed list content"
    );

    // The catalog entry reaches the same import flow. Use real clipboard
    // paste, including CRLF, Chinese names and a copied-table tab separator.
    click_list_control(cx, id, "stock-list-add")?;
    click_list_control(cx, id, "stock-catalog-import")?;
    ensure!(
        stock_import_value(cx, id)?.is_empty(),
        "Reopening import retained its cancelled draft"
    );
    let text = "AAPL 苹果\r\n00700 腾讯控股\r\naapl\r\nTSLA\t特斯拉\n09988 阿里巴巴\nSH603598 引力传媒\n001318 阳光乳业\n603598.SH\n600520\nGOOG";
    paste_stock_import(cx, id, text)?;
    expect_import_summary(cx, id, "可添加 5 支 · 已有 1 支 · 重复 2 次 · 未收录 2 项")?;
    ensure!(
        current_panel(cx, id)?.content == before.content
            && current_panel(cx, id)?.model == before.model,
        "Pasting a mixed stock table mutated the group before confirmation"
    );
    let handle = native_workspace(cx, owner)?;
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("workspace");
    cx.capture_screenshot(handle)
        .context("The stock import preview did not render")?
        .save(output.with_file_name(format!("{stem}-stock-import.png")))?;

    click_list_control(cx, id, "stock-import-confirm")?;
    let imported = current_panel(cx, id)?;
    let members = [
        Symbol::Tencent,
        Symbol::Aapl,
        Symbol::Tsla,
        Symbol::Alibaba,
        Symbol::Yinli,
        Symbol::Yangguang,
    ];
    let mut expected_content = before.content.clone();
    expected_content["members"] = serde_json::to_value(members)?;
    ensure!(
        list_members(&imported)? == members
            && imported.model == before.model
            && imported.content == expected_content
            && current_panel(cx, peer.model.id)?.content == peer_before.content
            && current_panel(cx, peer.model.id)?.model == peer_before.model
            && current_panel(cx, chart.model.id)?.content == chart_before.content
            && current_panel(cx, chart.model.id)?.model == chart_before.model,
        "Confirming import lost ordering, changed filters/selection/name, or affected a linked peer"
    );
    cx.update_window(handle, |_, window, _| {
        ensure!(
            window
                .within(format!("stock-panel-{id}"))
                .find("stock-import-message")
                .label()
                == Some("已添加 5 支股票到当前列表"),
            "Successful import did not report how many stocks were added"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    expect_import_summary(cx, id, "可添加 0 支 · 已有 6 支 · 重复 2 次 · 未收录 2 项")?;
    // Disabled button metadata is not supplied by GPUI Button; exercise the
    // actual click and observe that a repeated confirmation is harmless.
    click_list_control(cx, id, "stock-import-confirm")?;
    ensure!(
        current_panel(cx, id)?.content == imported.content,
        "Repeated import duplicated stocks"
    );
    click_list_control(cx, id, "stock-import-cancel")?;

    // Editing the text replaces the pending candidate set. An all-unknown
    // replacement must never submit a recognized candidate from older text.
    click_list_control(cx, id, "stock-list-import")?;
    ensure!(
        stock_import_value(cx, id)?.is_empty(),
        "Confirmed import retained a draft on reopen"
    );
    paste_stock_import(cx, id, "NVDA")?;
    expect_import_summary(cx, id, "可添加 1 支 · 已有 0 支 · 重复 0 次 · 未收录 0 项")?;
    paste_stock_import(cx, id, "600520")?;
    expect_import_summary(cx, id, "可添加 0 支 · 已有 0 支 · 重复 0 次 · 未收录 1 项")?;
    click_list_control(cx, id, "stock-import-confirm")?;
    ensure!(
        current_panel(cx, id)?.content == imported.content,
        "An all-unknown preview committed stale recognized stocks"
    );
    click_list_control(cx, id, "stock-import-clear")?;
    ensure!(
        stock_import_value(cx, id)?.is_empty(),
        "Clear did not empty the import editor"
    );
    click_list_control(cx, id, "stock-import-confirm")?;
    click_list_control(cx, id, "stock-import-cancel")?;

    // Move the component while a new draft is open: only committed member
    // configuration may travel with the rebuilt, window-bound input state.
    click_list_control(cx, id, "stock-list-import")?;
    paste_stock_import(cx, id, "MSFT")?;
    cx.update(|cx| workspace::return_window(owner, cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        current_panel(cx, id)?.content == imported.content,
        "Redocking changed imported membership"
    );
    click_list_control(cx, id, "stock-list-import")?;
    ensure!(
        stock_import_value(cx, id)?.is_empty(),
        "A pending import draft crossed native windows"
    );
    click_list_control(cx, id, "stock-import-cancel")?;
    let saved_workspace = cx.update(|cx| workspace::snapshot(cx));
    cx.update(|cx| workspace::switch_board(original_board, cx))?;
    cx.update(|cx| workspace::switch_board(board, cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        same_board_workspace(&saved_workspace, &cx.update(|cx| workspace::snapshot(cx)))?,
        "Switching boards lost imported members or saved an import draft"
    );
    let saved = cx.update(|cx| workspace::snapshot_board_set(cx));
    let directory = BoardSmokeDirectory::new()?;
    let path = directory.0.join("stock-import.json");
    storage::write_board_set(&path, &saved)?;
    let loaded = storage::read_board_set(&path)?;
    cx.update(|cx| workspace::restore_board_set(loaded, cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        current_panel(cx, id)?.content == imported.content
            && current_panel(cx, id)?.model == imported.model
            && current_panel(cx, peer.model.id)?.content == peer_before.content,
        "Saving/reloading a bulk-imported group changed members, metadata or its peer"
    );
    Ok(())
}

fn paste_stock_import(cx: &mut HeadlessAppContext, id: u64, text: &str) -> Result<()> {
    let handle = panel_window(cx, id)?;
    cx.update_window(handle, |_, window, cx| {
        window
            .within(format!("stock-panel-{id}"))
            .click("stock-import-text", cx);
        window.press("ctrl-a", cx);
        window.press("backspace", cx);
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_owned()));
        window.press("ctrl-v", cx);
    })?;
    render_workspace_windows(cx)?;
    ensure!(
        stock_import_value(cx, id)? == text,
        "The native multiline import input did not preserve the pasted stock table"
    );
    Ok(())
}

fn stock_import_value(cx: &mut HeadlessAppContext, id: u64) -> Result<String> {
    let handle = panel_window(cx, id)?;
    cx.update_window(handle, |_, window, _| {
        let container = window
            .within(format!("stock-panel-{id}"))
            .find("stock-import-text");
        // Textarea uses its entity ID internally. Inspect its actual input
        // below our observed wrapper, whose own native value is intentionally
        // absent; do not turn a missing value into a false empty-input pass.
        let values: Vec<String> = gpui_kit::base::test_support::snapshots(window)
            .into_iter()
            .filter(|node| node.path().starts_with(container.path()))
            .filter_map(|node| node.value().map(str::to_owned))
            .collect();
        ensure!(
            values.len() == 1,
            "The import editor has no unique native input value"
        );
        Ok(values[0].clone())
    })?
}

fn expect_import_summary(cx: &mut HeadlessAppContext, id: u64, expected: &str) -> Result<()> {
    let handle = panel_window(cx, id)?;
    cx.update_window(handle, |_, window, _| {
        let summary = window.within(format!("stock-panel-{id}")).find("stock-import-summary");
        ensure!(
            summary.label() == Some(expected),
            "The import preview statistics do not match its current text: expected={expected}, actual={:?}",
            summary.label()
        );
        Ok(())
    })?
}

/// Inspect the rendered daily candle, then open its own frozen intraday
/// window. These interactions must not become workspace layout or symbol edits.
fn verify_chart_inspection(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
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

fn verify_chart_interactions(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
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

fn verify_historical_intraday_toggle(
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

fn ensure_no_candle_card(
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

fn expected_daily_index(panel: &SavedPanel, fraction: f32) -> Result<usize> {
    let viewport: crate::chart::ChartViewport =
        serde_json::from_value(panel.content["chart"].clone())?;
    let length = market::bars(panel.model.symbol, Period::Day, 0).len();
    let count = viewport.bars_visible.min(length);
    let end = length - viewport.offset_from_latest.min(length - count);
    Ok(end - count + (count as f32 * fraction).floor() as usize)
}

fn click_daily_fraction(
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

fn verify_candle_card(
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

fn compact_chart_number(value: u64) -> String {
    if value >= 100_000_000 {
        format!("{:.2}亿", value as f64 / 100_000_000.)
    } else if value >= 10_000 {
        format!("{:.2}万", value as f64 / 10_000.)
    } else {
        value.to_string()
    }
}

fn historical_windows(cx: &mut HeadlessAppContext) -> Result<Vec<AnyWindowHandle>> {
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

fn verify_historical_date(
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

fn save_chart_screenshot(
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

/// Names and stock membership belong to the component instance. In particular,
/// a linked quote selection must never replace another list's own members.
fn verify_stock_lists(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let original = cx.update(|cx| workspace::snapshot_board_set(cx));
    let result = verify_stock_list_interactions(cx, output);
    let restored = cx.update(|cx| workspace::restore_board_set(original, cx));
    cx.run_until_parked();
    result?;
    restored?;
    render_workspace_windows(cx)?;
    println!(
        "PASS: component rename/cancel/default/validation; independent stock membership, add/remove/duplicate rejection and scoped search/sort; symbol-only linking, copy isolation, redocking and board save/reload"
    );
    Ok(())
}

fn verify_stock_list_interactions(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let original_board = cx.update(|cx| workspace::active_board_id(cx));
    let board =
        cx.update(|cx| workspace::create_board("股票分组", workspace::BoardPreset::Empty, cx))?;
    let (hk_owner, hk) = add_floating_panel(cx, PanelKind::Watchlist)?;
    let (_, tech) = add_floating_panel(cx, PanelKind::Watchlist)?;
    let (chart_owner, chart) = add_floating_panel(cx, PanelKind::Chart)?;
    let hk_id = hk.model.id;
    let tech_id = tech.model.id;
    let hk_handle = native_workspace(cx, hk_owner)?;

    begin_panel_rename(cx, hk_id)?;
    fill_panel_name(cx, hk_id, &"名".repeat(33))?;
    cx.update_window(hk_handle, |_, window, cx| {
        window.click(format!("panel-name-confirm-{hk_id}"), cx);
    })?;
    render_workspace_windows(cx)?;
    cx.update_window(hk_handle, |_, window, cx| {
        ensure!(
            window.find(format!("panel-name-input-{hk_id}")).visible(),
            "An invalid component name closed its editor"
        );
        window.click(format!("panel-name-cancel-{hk_id}"), cx);
        Ok::<_, anyhow::Error>(())
    })??;
    ensure!(
        current_panel(cx, hk_id)?.model.custom_name.is_none(),
        "Rejecting and cancelling a name modified the component"
    );
    rename_panel(cx, hk_id, "  港股观察  ")?;
    let renamed = current_panel(cx, hk_id)?;
    ensure!(
        renamed.model.title() == "港股观察",
        "The trimmed component name did not update its model: {:?}",
        renamed.model.custom_name
    );
    // HeadlessAppContext uses TestWindow, whose get_title inherits the empty
    // PlatformWindow default despite set_title storing a private value. Check
    // the actual Dock title here; OS titles are verified on the native desktop.
    cx.update_window(hk_handle, |_, window, _| {
        let title = window.find(format!("panel-title-{hk_id}"));
        ensure!(
            title.visible() && title.label() == Some("港股观察"),
            "The renamed component did not update its rendered Dock title: {:?}",
            title.label()
        );
        Ok::<_, anyhow::Error>(())
    })??;
    begin_panel_rename(cx, hk_id)?;
    fill_panel_name(cx, hk_id, "取消后的名字")?;
    cx.update_window(hk_handle, |_, window, cx| window.press("escape", cx))?;
    ensure!(
        current_panel(cx, hk_id)?.model.title() == "港股观察",
        "Escape committed a pending component name"
    );
    rename_panel(cx, tech_id, "临时分组")?;
    rename_panel(cx, tech_id, "  ")?;
    ensure!(
        current_panel(cx, tech_id)?.model.custom_name.is_none()
            && current_panel(cx, tech_id)?.model.title() == "股票列表",
        "An empty component name did not restore its default title"
    );
    rename_panel(cx, tech_id, "科技股")?;
    rename_panel(cx, chart.model.id, "联动行情")?;
    // The fixture deliberately exercises USD symbols; new components now
    // start on an A-share, so select AAPL through the real linked list first.
    click_list_control(cx, tech_id, "symbol-AAPL")?;

    // Remove the selected stock as well as all other rows. Removing a child
    // action must not bubble to the row and emit an unintended symbol change.
    for symbol in Symbol::ALL {
        click_list_control(cx, hk_id, &format!("stock-list-remove-{}", symbol.code()))?;
    }
    let empty = cx.update(|cx| workspace::snapshot(cx));
    ensure!(
        list_members(&current_panel(cx, hk_id)?)?.is_empty()
            && board_panels(&empty)?
                .iter()
                .all(|(_, p)| p.model.symbol == Symbol::Aapl)
            && list_members(&current_panel(cx, tech_id)?)? == Symbol::ALL,
        "Removing stocks selected a row, reset the empty list or modified its peer"
    );
    cx.update_window(hk_handle, |_, window, _| {
        ensure!(
            window
                .within(format!("stock-panel-{hk_id}"))
                .find("stock-list-empty-add")
                .visible(),
            "An empty stock list has no add-stock action"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    // A pre-grouping layout keeps its original six members and saved symbol;
    // introducing new A-share fixtures must not silently enlarge old lists.
    let mut legacy = empty.clone();
    let mut stripped = false;
    for window in &mut legacy.windows {
        stripped |= remove_legacy_members(&mut window.dock.center, hk_id)?;
    }
    ensure!(
        stripped,
        "The legacy stock-list fixture had no member field"
    );
    cx.update(|cx| workspace::restore(legacy, cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        list_members(&current_panel(cx, hk_id)?)? == Symbol::LEGACY_DEFAULTS
            && current_panel(cx, hk_id)?.model.symbol == Symbol::Aapl
            && list_members(&current_panel(cx, tech_id)?)? == Symbol::ALL,
        "Restoring an old list changed its saved symbol or replaced its six legacy members"
    );
    // Explicit [] must still remain empty, unlike that legacy missing field.
    cx.update(|cx| workspace::restore(empty, cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        list_members(&current_panel(cx, hk_id)?)?.is_empty(),
        "Restoring an explicitly empty list repopulated the demo stocks"
    );
    click_list_control(cx, hk_id, "stock-list-empty-add")?;
    set_list_input(cx, hk_id, "stock-list-catalog-search", "腾讯")?;
    click_list_control(cx, hk_id, "stock-list-add-00700")?;
    let hk_handle = panel_window(cx, hk_id)?;
    let before_duplicate_add = current_panel(cx, hk_id)?;
    cx.update_window(hk_handle, |_, window, cx| {
        let mut scope = window.within(format!("stock-panel-{hk_id}"));
        // GPUI's Button disables interaction but does not expose aria-disabled;
        // test the visible state and the result of a real pointer click instead.
        ensure!(
            scope.find("stock-list-add-00700").label() == Some("已添加"),
            "The catalog did not identify its already-added stock"
        );
        scope.click("stock-list-add-00700", cx);
        Ok::<_, anyhow::Error>(())
    })??;
    render_workspace_windows(cx)?;
    let after_duplicate_add = current_panel(cx, hk_id)?;
    ensure!(
        list_members(&after_duplicate_add)? == [Symbol::Tencent]
            && after_duplicate_add.model == before_duplicate_add.model
            && after_duplicate_add.content == before_duplicate_add.content,
        "Clicking an already-added stock duplicated membership or changed component state"
    );
    set_list_input(cx, hk_id, "stock-list-catalog-search", "09988")?;
    click_list_control(cx, hk_id, "stock-list-add-09988")?;
    click_list_control(cx, hk_id, "stock-list-back")?;
    ensure!(
        list_members(&current_panel(cx, hk_id)?)? == [Symbol::Tencent, Symbol::Alibaba]
            && current_panel(cx, hk_id)?.model.symbol == Symbol::Aapl,
        "Catalog add duplicated a member or unexpectedly selected the added stock"
    );
    let technology = [Symbol::Aapl, Symbol::Msft, Symbol::Nvda];
    for symbol in Symbol::ALL
        .into_iter()
        .filter(|symbol| !technology.contains(symbol))
    {
        click_list_control(cx, tech_id, &format!("stock-list-remove-{}", symbol.code()))?;
    }
    ensure!(
        list_members(&current_panel(cx, tech_id)?)? == technology,
        "The technology list did not retain its own membership"
    );

    set_list_input(cx, hk_id, "watchlist-search", "没有匹配股票")?;
    let hk_handle = panel_window(cx, hk_id)?;
    cx.update_window(hk_handle, |_, window, _| {
        ensure!(
            window
                .within(format!("stock-panel-{hk_id}"))
                .try_find("symbol-00700")
                .is_none(),
            "A nonmatching query did not filter the current list"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    set_list_input(cx, hk_id, "watchlist-search", "腾讯")?;
    click_list_control(cx, hk_id, "watchlist-sort")?;
    click_list_control(cx, hk_id, "stock-list-add")?;
    set_list_input(cx, hk_id, "stock-list-catalog-search", "苹果")?;
    click_list_control(cx, hk_id, "stock-list-back")?;
    ensure!(
        current_panel(cx, hk_id)?.content["query"] == "腾讯"
            && current_panel(cx, hk_id)?.content["sort"] == 1
            && current_panel(cx, tech_id)?.content["query"] == ""
            && current_panel(cx, tech_id)?.content["sort"] == 0,
        "Member filtering, catalog searching or sorting leaked into another list"
    );
    click_list_control(cx, tech_id, "symbol-MSFT")?;
    let linked = cx.update(|cx| workspace::snapshot(cx));
    ensure!(
        board_panels(&linked)?
            .iter()
            .all(|(_, p)| p.model.symbol == Symbol::Msft)
            && list_members(&current_panel(cx, hk_id)?)? == [Symbol::Tencent, Symbol::Alibaba]
            && list_members(&current_panel(cx, tech_id)?)? == technology,
        "Linking stocks replaced list membership or failed to update the chart"
    );

    let before_copy = cx.update(|cx| workspace::snapshot(cx));
    cx.update(|cx| workspace::duplicate(tech_id, cx))?;
    render_workspace_windows(cx)?;
    let (copy_owner, copied) = board_panels(&cx.update(|cx| workspace::snapshot(cx)))?
        .into_iter()
        .find(|(owner, _)| !before_copy.windows.iter().any(|w| w.id == *owner))
        .context("Duplicating the stock list did not create an independent component")?;
    ensure!(
        copied.model.id != tech_id
            && copied.model.title() == "科技股"
            && copied.content == current_panel(cx, tech_id)?.content,
        "Duplicating a list lost its name/content or reused its identity"
    );
    click_list_control(cx, copied.model.id, "stock-list-remove-NVDA")?;
    rename_panel(cx, copied.model.id, "核心科技")?;
    ensure!(
        list_members(&current_panel(cx, copied.model.id)?)? == [Symbol::Aapl, Symbol::Msft]
            && list_members(&current_panel(cx, tech_id)?)? == technology
            && current_panel(cx, tech_id)?.model.title() == "科技股",
        "Editing a copied stock list changed its source"
    );
    cx.update(|cx| workspace::dismiss_window(copy_owner, cx))?;

    // Rebuild the inputs in another native window, then leave the HK list
    // floating while switching boards and saving the entire board collection.
    cx.update(|cx| workspace::return_window(chart_owner, cx))?;
    cx.update(|cx| {
        workspace::move_between(tech_id, 0, Some((chart.model.id, DockSide::Right)), cx)
    })?;
    cx.update(|cx| workspace::move_between(hk_id, 0, Some((chart.model.id, DockSide::Left)), cx))?;
    render_workspace_windows(cx)?;
    let before_detach = current_panel(cx, hk_id)?;
    cx.update(|cx| workspace::detach(hk_id, None, cx))?;
    render_workspace_windows(cx)?;
    let detached = current_panel(cx, hk_id)?;
    ensure!(
        detached.model == before_detach.model && detached.content == before_detach.content,
        "Moving the named list between native windows lost its members or filters"
    );
    let named_state = cx.update(|cx| workspace::snapshot(cx));
    let floating = panel_window(cx, hk_id)?;
    cx.update(|cx| workspace::switch_board(original_board, cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        cx.update_window(floating, |_, _, _| ()).is_err(),
        "Switching boards left a named floating list mounted"
    );
    cx.update(|cx| workspace::switch_board(board, cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        same_board_workspace(&named_state, &cx.update(|cx| workspace::snapshot(cx)))?,
        "A board switch changed component names, members or search state"
    );
    let saved = cx.update(|cx| workspace::snapshot_board_set(cx));
    let directory = BoardSmokeDirectory::new()?;
    let path = directory.0.join("stock-lists.json");
    storage::write_board_set(&path, &saved)?;
    let loaded = storage::read_board_set(&path)?;
    cx.update(|cx| workspace::switch_board(original_board, cx))?;
    cx.update(|cx| workspace::restore_board_set(loaded, cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        cx.update(|cx| workspace::active_board_id(cx)) == board
            && same_board_workspace(&named_state, &cx.update(|cx| workspace::snapshot(cx)))?,
        "Saving/reloading named stock groups lost membership or floating layout"
    );
    cx.update(|cx| workspace::move_between(hk_id, 0, Some((chart.model.id, DockSide::Left)), cx))?;
    render_workspace_windows(cx)?;
    set_list_input(cx, hk_id, "watchlist-search", "")?;
    // The migration assertions above intentionally exercise repeated splits.
    // Give the product preview readable widths via the same persisted-layout
    // restore path, keeping the actual tested component models and contents.
    let mut preview = cx.update(|cx| workspace::snapshot(cx));
    let panels = board_panels(&preview)?;
    let main_state = preview
        .windows
        .iter_mut()
        .find(|window| window.id == 0)
        .context("The stock-list preview has no main window")?;
    let mut children = Vec::new();
    for id in [hk_id, chart.model.id, tech_id] {
        let panel = &panels
            .iter()
            .find(|(_, panel)| panel.model.id == id)
            .context("The stock-list preview lost a component")?
            .1;
        children.push(PanelState {
            panel_name: "TabPanel".into(),
            children: vec![PanelState {
                panel_name: "tydaro.panel".into(),
                children: vec![],
                info: PanelInfo::Panel(serde_json::to_value(panel)?),
            }],
            info: PanelInfo::Tabs { active_index: 0 },
        });
    }
    main_state.dock.center = PanelState {
        panel_name: "StackPanel".into(),
        children,
        info: PanelInfo::Stack {
            axis: 0,
            sizes: vec![
                px(290.),
                px((main_state.bounds[2] - 702.).max(500.)),
                px(290.),
            ],
        },
    };
    cx.update(|cx| workspace::restore(preview, cx))?;
    render_workspace_windows(cx)?;
    let main = native_workspace(cx, 0)?;
    cx.update_window(main, |_, window, _| {
        ensure!(
            window
                .within(format!("stock-panel-{hk_id}"))
                .find("symbol-00700")
                .visible()
                && window
                    .within(format!("stock-panel-{tech_id}"))
                    .find("symbol-MSFT")
                    .visible()
                && window.find("native-market-chart").visible(),
            "The named-list preview does not show both independent groups and their linked chart"
        );
        ensure!(
            window.find("native-market-chart").bounds().size.width >= px(500.)
                && window
                    .find(format!("stock-panel-{hk_id}"))
                    .bounds()
                    .size
                    .width
                    >= px(250.)
                && window
                    .find(format!("stock-panel-{tech_id}"))
                    .bounds()
                    .size
                    .width
                    >= px(250.),
            "The stock-list preview did not allocate readable list and chart widths"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    save_board_screenshot(cx, output, "stock-lists")?;
    Ok(())
}

fn current_panel(cx: &mut HeadlessAppContext, id: u64) -> Result<SavedPanel> {
    Ok(unique_panel(&cx.update(|cx| workspace::snapshot(cx)), id)?.1)
}

fn panel_window(cx: &mut HeadlessAppContext, id: u64) -> Result<AnyWindowHandle> {
    let owner = unique_panel(&cx.update(|cx| workspace::snapshot(cx)), id)?.0;
    native_workspace(cx, owner)
}

fn list_members(panel: &SavedPanel) -> Result<Vec<Symbol>> {
    serde_json::from_value(panel.content["members"].clone())
        .context("The stock-list snapshot has no valid member list")
}

fn remove_legacy_members(state: &mut PanelState, id: u64) -> Result<bool> {
    if let PanelInfo::Panel(value) = &mut state.info
        && state.panel_name == "tydaro.panel"
    {
        let mut panel: SavedPanel = serde_json::from_value(value.clone())?;
        if panel.model.id == id {
            let removed = panel
                .content
                .as_object_mut()
                .context("The legacy fixture has no component content")?
                .remove("members")
                .is_some();
            *value = serde_json::to_value(panel)?;
            return Ok(removed);
        }
    }
    let mut removed = false;
    for child in &mut state.children {
        removed |= remove_legacy_members(child, id)?;
    }
    Ok(removed)
}

fn begin_panel_rename(cx: &mut HeadlessAppContext, id: u64) -> Result<()> {
    let handle = panel_window(cx, id)?;
    cx.update_window(handle, |_, window, cx| {
        window.click(format!("panel-menu-{id}"), cx);
        ensure!(
            window.within("popup-menu").find(0usize).label() == Some("重命名组件"),
            "The component menu has no rename action"
        );
        window.within("popup-menu").click(0usize, cx);
        Ok::<_, anyhow::Error>(())
    })??;
    render_workspace_windows(cx)
}

fn fill_panel_name(cx: &mut HeadlessAppContext, id: u64, name: &str) -> Result<()> {
    let handle = panel_window(cx, id)?;
    cx.update_window(handle, |_, window, cx| {
        window.click(format!("panel-name-input-{id}"), cx);
        window.press("ctrl-a", cx);
        window.press("backspace", cx);
        window.input(name, cx);
        ensure!(
            window.find(format!("panel-name-input-{id}")).value() == Some(name),
            "The rename input did not receive the requested value: requested={name:?}, actual={:?}",
            window.find(format!("panel-name-input-{id}")).value()
        );
        Ok::<_, anyhow::Error>(())
    })??;
    Ok(())
}

fn rename_panel(cx: &mut HeadlessAppContext, id: u64, name: &str) -> Result<()> {
    begin_panel_rename(cx, id)?;
    fill_panel_name(cx, id, name)?;
    let handle = panel_window(cx, id)?;
    cx.update_window(handle, |_, window, cx| window.press("enter", cx))?;
    render_workspace_windows(cx)
}

fn click_list_control(cx: &mut HeadlessAppContext, id: u64, control: &str) -> Result<()> {
    let handle = panel_window(cx, id)?;
    cx.update_window(handle, |_, window, cx| {
        window
            .within(format!("stock-panel-{id}"))
            .click(control.to_owned(), cx);
    })?;
    render_workspace_windows(cx)
}

fn set_list_input(cx: &mut HeadlessAppContext, id: u64, control: &str, text: &str) -> Result<()> {
    let handle = panel_window(cx, id)?;
    cx.update_window(handle, |_, window, cx| {
        window
            .within(format!("stock-panel-{id}"))
            .click(control.to_owned(), cx);
        window.press("ctrl-a", cx);
        window.press("backspace", cx);
        window.input(text, cx);
    })?;
    render_workspace_windows(cx)
}

/// Keep multiple complete workspaces alive across switches, including their
/// native floating windows, content and independent layout undo histories.
fn verify_boards(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let original = cx.update(|cx| workspace::snapshot_board_set(cx));
    let result = verify_board_interactions(cx, output);
    let restored = cx.update(|cx| workspace::restore_board_set(original, cx));
    cx.run_until_parked();
    result?;
    restored?;
    render_workspace_windows(cx)?;
    println!(
        "PASS: named board UI, independent symbols/forms/viewports/floating windows and undo, copy identity isolation, reorder/delete, schema 2 save/reload"
    );
    Ok(())
}

fn verify_board_interactions(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let board_a = cx.update(|cx| workspace::active_board_id(cx));
    cx.update(|cx| workspace::rename_board(board_a, "趋势看盘", cx))?;
    let main = native_workspace(cx, 0)?;
    render_workspace_windows(cx)?;
    cx.update_window(main, |_, window, cx| window.click("book-queue", cx))?;
    cx.update(|cx| workspace::add_panel(0, PanelKind::OrderEntry, cx))?;
    render_workspace_windows(cx)?;
    let with_entry = cx.update(|cx| workspace::snapshot(cx));
    let (entry_owner, entry) = board_panels(&with_entry)?
        .into_iter()
        .find(|(owner, panel)| *owner != 0 && panel.model.kind == PanelKind::OrderEntry)
        .context("Board A has no independent order form")?;
    let entry_handle = native_workspace(cx, entry_owner)?;
    cx.update_window(entry_handle, |_, window, cx| {
        window.click("order-price", cx);
        window.press("ctrl-a", cx);
        window.input("421.50", cx);
        window.click("order-quantity", cx);
        window.press("ctrl-a", cx);
        window.input("325", cx);
    })?;
    let chart = board_panels(&with_entry)?
        .into_iter()
        .find(|(_, panel)| panel.model.kind == PanelKind::Chart)
        .context("Board A has no chart")?
        .1;
    ensure!(
        chart.model.symbol == Symbol::Msft && chart.model.period == Period::Day,
        "The board test did not retain the earlier native chart interaction"
    );
    // Establish the preceding arrangement through the production persistence
    // path; the headless test has no autosave timer to record it for undo.
    cx.update(workspace::verify_workspace_persistence)?;
    cx.update(|cx| workspace::detach(chart.model.id, None, cx))?;
    render_workspace_windows(cx)?;
    let state_a = cx.update(|cx| workspace::snapshot(cx));
    let (chart_owner, _) = unique_panel(&state_a, chart.model.id)?;
    let a_floating = native_workspace(cx, chart_owner)?;
    ensure!(
        unique_panel(&state_a, entry.model.id)?.1.content["quantity"] == "325"
            && unique_panel(&state_a, entry.model.id)?.1.content["price"] == "421.50"
            && board_panels(&state_a)?.iter().any(|(_, p)| {
                p.model.kind == PanelKind::OrderBook && p.content["book_queue"] == true
            }),
        "Native form or order-book edits did not enter the board snapshot"
    );

    // Both cancelling and confirming travel through the actual + menu and
    // inline input, not the board creation helper.
    begin_empty_board(cx)?;
    cx.update_window(main, |_, window, cx| {
        window.click("board-name-input", cx);
        window.press("ctrl-a", cx);
        window.input("不应创建", cx);
        window.click("board-name-cancel", cx);
    })?;
    cx.run_until_parked();
    ensure!(
        cx.update(|cx| workspace::board_summaries(cx)).len() == 1,
        "Cancelling the board name created a board"
    );
    begin_empty_board(cx)?;
    cx.update_window(main, |_, window, cx| {
        window.click("board-name-input", cx);
        window.press("ctrl-a", cx);
        window.input("竞价观察", cx);
        window.click("board-name-confirm", cx);
    })?;
    cx.run_until_parked();
    render_workspace_windows(cx)?;
    let board_b = cx.update(|cx| workspace::active_board_id(cx));
    ensure!(
        board_b != board_a,
        "Confirming the new board did not select it"
    );
    let empty = cx.update(|cx| workspace::snapshot(cx));
    ensure!(
        board_panels(&empty)?.is_empty() && empty.windows.len() == 1,
        "An empty board inherited components or floating windows"
    );
    ensure!(
        cx.update_window(a_floating, |_, _, _| ()).is_err(),
        "The inactive board's native floating window remained open"
    );
    save_board_screenshot(cx, output, "empty-board")?;

    // Build a genuinely different board from independent components, then link
    // another symbol. The hidden board must not receive this link-group event.
    cx.update(|cx| workspace::add_panel(0, PanelKind::Watchlist, cx))?;
    let watchlist = board_panels(&cx.update(|cx| workspace::snapshot(cx)))?
        .into_iter()
        .next()
        .context("Empty board did not accept a new component")?;
    cx.update(|cx| workspace::return_window(watchlist.0, cx))?;
    cx.update(|cx| workspace::add_panel(0, PanelKind::Quote, cx))?;
    let quote = board_panels(&cx.update(|cx| workspace::snapshot(cx)))?
        .into_iter()
        .find(|(_, p)| p.model.kind == PanelKind::Quote)
        .context("Board B quote was not added")?
        .1;
    cx.update(|cx| {
        workspace::move_between(
            quote.model.id,
            0,
            Some((watchlist.1.model.id, DockSide::Right)),
            cx,
        )
    })?;
    render_workspace_windows(cx)?;
    cx.update_window(main, |_, window, cx| window.click("symbol-TSLA", cx))?;
    render_workspace_windows(cx)?;
    let state_b = cx.update(|cx| workspace::snapshot(cx));
    ensure!(
        board_panels(&state_b)?
            .iter()
            .all(|(_, p)| p.model.symbol == Symbol::Tsla),
        "Board B's own linked components did not follow TSLA"
    );
    click_board_tab(cx, board_a)?;
    ensure!(
        same_board_workspace(&state_a, &cx.update(|cx| workspace::snapshot(cx)))?,
        "Switching back lost Board A's layout, form, chart or floating content"
    );
    let reopened = native_workspace(cx, chart_owner)?;
    ensure!(
        reopened != a_floating,
        "A board reused the closed native window instead of rebuilding its content"
    );
    click_board_tab(cx, board_b)?;
    ensure!(
        same_board_workspace(&state_b, &cx.update(|cx| workspace::snapshot(cx)))?,
        "Board B changed while Board A was selected"
    );

    // The last A operation detached its chart; the last B operation adds a
    // chart. Undoing each after a switch must affect only that board's history.
    cx.update(|cx| workspace::add_panel(0, PanelKind::Chart, cx))?;
    let b_with_chart = cx.update(|cx| workspace::snapshot(cx));
    click_board_tab(cx, board_a)?;
    cx.update(workspace::undo);
    render_workspace_windows(cx)?;
    let a_undone = cx.update(|cx| workspace::snapshot(cx));
    ensure!(
        unique_panel(&a_undone, chart.model.id)?.0 == 0
            && unique_panel(&a_undone, entry.model.id)?.1.content["quantity"] == "325",
        "Board A undo used another board's history or reverted live form state"
    );
    click_board_tab(cx, board_b)?;
    ensure!(
        same_board_workspace(&b_with_chart, &cx.update(|cx| workspace::snapshot(cx)))?,
        "Undoing Board A modified Board B"
    );
    cx.update(workspace::undo);
    render_workspace_windows(cx)?;
    ensure!(
        same_board_workspace(&state_b, &cx.update(|cx| workspace::snapshot(cx)))?,
        "Board B undo did not restore its own previous arrangement"
    );

    click_board_tab(cx, board_a)?;
    let source = cx.update(|cx| workspace::snapshot(cx));
    let board_c = cx.update(|cx| workspace::duplicate_board("盘口联动", cx))?;
    render_workspace_windows(cx)?;
    let copied = cx.update(|cx| workspace::snapshot(cx));
    let source_panels = board_panels(&source)?;
    let copied_panels = board_panels(&copied)?;
    ensure!(
        source_panels.len() == copied_panels.len(),
        "Board copy lost a component"
    );
    for ((_, expected), (_, actual)) in source_panels.iter().zip(&copied_panels) {
        ensure!(
            !source_panels
                .iter()
                .any(|(_, p)| p.model.id == actual.model.id),
            "Copied board reused a source panel identity"
        );
        let mut model = actual.model.clone();
        model.id = expected.model.id;
        ensure!(
            model == expected.model && actual.content == expected.content,
            "Copied board changed its component model or content"
        );
    }
    ensure!(
        copied
            .windows
            .iter()
            .filter(|w| w.id != 0)
            .all(|w| { source.windows.iter().all(|original| original.id != w.id) }),
        "Copied board reused an independent window identity"
    );
    let (copied_owner, copied_entry) = copied_panels
        .iter()
        .find(|(owner, panel)| *owner != 0 && panel.model.kind == PanelKind::OrderEntry)
        .context("Copied board lost its independent order form")?;
    let copied_entry_handle = native_workspace(cx, *copied_owner)?;
    cx.update_window(copied_entry_handle, |_, window, cx| {
        window.click("order-quantity", cx);
        window.press("ctrl-a", cx);
        window.input("650", cx);
    })?;
    click_board_tab(cx, board_a)?;
    ensure!(
        unique_panel(&cx.update(|cx| workspace::snapshot(cx)), entry.model.id)?
            .1
            .content["quantity"]
            == "325",
        "Editing the copied board changed its source form"
    );
    click_board_tab(cx, board_c)?;
    ensure!(
        unique_panel(
            &cx.update(|cx| workspace::snapshot(cx)),
            copied_entry.model.id,
        )?
        .1
        .content["quantity"]
            == "650",
        "The copied form did not keep its own edited value"
    );
    cx.update(|cx| workspace::rename_board(board_c, "多头鹰", cx))?;
    cx.update(|cx| workspace::move_board(board_c, -1, cx))?;
    let summaries = cx.update(|cx| workspace::board_summaries(cx));
    ensure!(
        summaries
            == vec![
                (board_a, "趋势看盘".into()),
                (board_c, "多头鹰".into()),
                (board_b, "竞价观察".into())
            ],
        "Renaming or moving a board changed the wrong tab"
    );
    let before_rejection = cx.update(|cx| workspace::snapshot_board_set(cx));
    ensure!(
        cx.update(|cx| workspace::rename_board(board_b, "趋势看盘", cx))
            .is_err()
            && cx
                .update(|cx| workspace::create_board("多头鹰", workspace::BoardPreset::Empty, cx))
                .is_err()
            && cx
                .update(|cx| workspace::switch_board(u64::MAX - 1, cx))
                .is_err()
            && cx
                .update(|cx| workspace::delete_board(u64::MAX - 1, cx))
                .is_err(),
        "Board management accepted a duplicate name or an unknown target"
    );
    ensure!(
        serde_json::to_value(&before_rejection)?
            == serde_json::to_value(cx.update(|cx| workspace::snapshot_board_set(cx)))?,
        "A rejected board command mutated the workspace"
    );
    // Restore the complete file while a different board is active. Both the
    // last selection and every inactive snapshot must come from the file.
    click_board_tab(cx, board_b)?;
    let saved = cx.update(|cx| workspace::snapshot_board_set(cx));
    let directory = BoardSmokeDirectory::new()?;
    let path = directory.0.join("workspace.json");
    storage::write_board_set(&path, &saved)?;
    let loaded = storage::read_board_set(&path)?;
    ensure!(
        serde_json::to_value(&saved)? == serde_json::to_value(&loaded)?,
        "Saving boards changed their ordered snapshots or selected tab"
    );
    click_board_tab(cx, board_a)?;
    cx.update(|cx| workspace::restore_board_set(loaded, cx))?;
    render_workspace_windows(cx)?;
    let reloaded = cx.update(|cx| workspace::snapshot_board_set(cx));
    ensure!(
        reloaded.active_board == board_b,
        "Reload did not restore the last selected board"
    );
    ensure!(
        reloaded.boards.len() == saved.boards.len(),
        "Reload lost inactive boards"
    );
    for (expected, actual) in saved.boards.iter().zip(&reloaded.boards) {
        ensure!(
            expected.id == actual.id
                && expected.name == actual.name
                && same_board_workspace(&expected.workspace, &actual.workspace)?,
            "Reload changed an active or inactive board"
        );
    }
    cx.update(|cx| workspace::delete_board(board_a, cx))?;
    cx.update(|cx| workspace::delete_board(board_c, cx))?;
    ensure!(
        cx.update(|cx| workspace::delete_board(board_b, cx))
            .is_err()
            && cx.update(|cx| workspace::board_summaries(cx)).len() == 1
            && cx.update(|cx| workspace::active_board_id(cx)) == board_b,
        "Deleting boards allowed the last remaining board to disappear"
    );

    // Render a fresh standard preset for the product preview after the state
    // assertions. Earlier detach/undo checks deliberately rearrange the chart.
    cx.update(|cx| workspace::restore_board_set(saved, cx))?;
    let standard =
        cx.update(|cx| workspace::create_board("综合看盘", workspace::BoardPreset::Standard, cx))?;
    render_workspace_windows(cx)?;
    let preset = cx.update(|cx| workspace::snapshot(cx));
    ensure!(
        cx.update(|cx| workspace::active_board_id(cx)) == standard
            && cx.update(|cx| workspace::board_summaries(cx)).len() >= 3
            && preset.windows.len() == 1
            && board_panels(&preset)?.len() == 9,
        "Creating the standard board did not activate its complete layout"
    );
    cx.update_window(main, |_, window, _| {
        ensure!(
            window.find("symbol-MSFT").visible()
                && window.find("native-market-chart").visible()
                && window.find("book-depth").visible(),
            "The standard board did not render its watchlist, chart and order book"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    save_board_screenshot(cx, output, "boards")?;
    Ok(())
}

fn native_workspace(cx: &mut HeadlessAppContext, id: u64) -> Result<AnyWindowHandle> {
    cx.update(|cx| workspace::workspace(id, cx).map(|w| w.read(cx).handle))
        .context("Board has no matching native workspace window")
}

fn begin_empty_board(cx: &mut HeadlessAppContext) -> Result<()> {
    let handle = native_workspace(cx, 0)?;
    cx.update_window(handle, |_, window, cx| {
        window.click("board-add", cx);
        ensure!(
            window.within("popup-menu").find(0usize).label() == Some("空白看板"),
            "The add-board menu has no empty-board action"
        );
        window.within("popup-menu").click(0usize, cx);
        Ok::<_, anyhow::Error>(())
    })??;
    cx.run_until_parked();
    render_workspace_windows(cx)
}

fn click_board_tab(cx: &mut HeadlessAppContext, id: u64) -> Result<()> {
    let handle = native_workspace(cx, 0)?;
    cx.update_window(handle, |_, window, cx| {
        let tab = format!("board-tab-{id}");
        ensure!(
            window.find(tab.clone()).visible(),
            "Board tab is not visible"
        );
        window.click(tab, cx);
        Ok::<_, anyhow::Error>(())
    })??;
    cx.run_until_parked();
    render_workspace_windows(cx)?;
    ensure!(
        cx.update(|cx| workspace::active_board_id(cx)) == id,
        "Clicking the native board tab did not switch boards"
    );
    Ok(())
}

fn save_board_screenshot(cx: &mut HeadlessAppContext, output: &Path, suffix: &str) -> Result<()> {
    let handle = native_workspace(cx, 0)?;
    let stem = output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("workspace");
    cx.capture_screenshot(handle)
        .context("The board did not render")?
        .save(output.with_file_name(format!("{stem}-{suffix}.png")))?;
    Ok(())
}

fn board_panels(state: &WorkspaceFile) -> Result<Vec<(u64, SavedPanel)>> {
    let mut result = Vec::new();
    for window in &state.windows {
        let mut panels = Vec::new();
        collect_panels(&window.dock.center, &mut panels)?;
        for dock in [
            &window.dock.left_dock,
            &window.dock.right_dock,
            &window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            collect_panels(dock.panel(), &mut panels)?;
        }
        result.extend(panels.into_iter().map(|panel| (window.id, panel)));
    }
    Ok(result)
}

fn same_board_workspace(a: &WorkspaceFile, b: &WorkspaceFile) -> Result<bool> {
    let normalize = |state: &WorkspaceFile| {
        let mut state = state.clone();
        // Entity allocation is process-wide and never rolls back. Native
        // positions may be clamped to the current display on reconstruction.
        state.next_id = 0;
        for window in &mut state.windows {
            window.bounds = [0.; 4];
        }
        serde_json::to_value(state)
    };
    Ok(normalize(a)? == normalize(b)?)
}

struct BoardSmokeDirectory(PathBuf);

impl BoardSmokeDirectory {
    fn new() -> Result<Self> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path =
            std::env::temp_dir().join(format!("tydaro-board-smoke-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for BoardSmokeDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Exercise the same production add/insert paths as the component menu. Each
/// insertion starts from the same workspace so a split cannot pass merely
/// because an unrelated earlier split happens to have the expected axis.
fn verify_new_floating_panels(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
    let original = cx.update(|cx| workspace::snapshot(cx));
    let mut original_panels = Vec::new();
    for window in &original.windows {
        collect_panels(&window.dock.center, &mut original_panels)?;
    }
    let target = original_panels
        .iter()
        .find(|p| p.model.kind == PanelKind::Chart)
        .context("No chart available as the insertion target")?
        .model
        .id;
    for (kind, side, image_name) in [
        (PanelKind::Quote, DockSide::Left, "quote"),
        (PanelKind::OrderBook, DockSide::Right, "order-book"),
        (PanelKind::Trades, DockSide::Top, "trades"),
        (PanelKind::TradeDetails, DockSide::Bottom, "trade-details"),
        (PanelKind::OrderDetails, DockSide::Center, "order-details"),
    ] {
        cx.update(|cx| workspace::add_panel(0, kind, cx))?;
        cx.run_until_parked();
        render_workspace_windows(cx)?;
        let added = cx.update(|cx| workspace::snapshot(cx));
        storage::validate(&added)?;
        ensure!(
            added.windows.len() == original.windows.len() + 1,
            "Adding {} did not create an independent window",
            kind.title()
        );
        let floating = added
            .windows
            .iter()
            .find(|window| !original.windows.iter().any(|old| old.id == window.id))
            .context("The new component's independent window is missing")?;
        let mut panels = Vec::new();
        collect_panels(&floating.dock.center, &mut panels)?;
        ensure!(
            panels.len() == 1,
            "New floating window has extra or hidden panels"
        );
        let added_panel = &panels[0];
        let id = added_panel.model.id;
        ensure!(added_panel.model.kind == kind, "Wrong component was added");
        ensure!(
            !original_panels.iter().any(|old| old.model.id == id),
            "Adding a component reused a persistent ID"
        );
        let handle = cx
            .update(|cx| workspace::workspace(floating.id, cx).map(|w| w.read(cx).handle))
            .context("The floating component has no native window")?;
        cx.update_window(handle, |_, window, _| {
            ensure!(
                window.find(format!("insert-panel-{id}")).visible(),
                "The new component has no visible insert action"
            );
            ensure!(
                window.find(format!("panel-drag-{id}")).visible(),
                "The component's drag header is not visible"
            );
            Ok::<_, anyhow::Error>(())
        })??;
        let image = cx
            .capture_screenshot(handle)
            .context("Floating component did not render")?;
        ensure!(
            image.pixels().filter(|p| p[0].abs_diff(p[1]) > 24).count() > 50,
            "Floating {} rendered no market content",
            kind.title()
        );
        let stem = output
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("workspace");
        image.save(output.with_file_name(format!("{stem}-{image_name}.png")))?;

        // Neither a locked destination nor a stale target may lose the source.
        let main = cx
            .update(|cx| workspace::workspace(0, cx))
            .context("Main window closed")?;
        let main_handle = cx.update(|cx| main.read(cx).handle);
        let main_dock = cx.update(|cx| main.read(cx).dock.clone());
        cx.update_window(main_handle, |_, window, cx| {
            main_dock.update(cx, |dock, cx| dock.set_locked(true, window, cx));
        })?;
        let locked = cx.update(|cx| workspace::snapshot(cx));
        ensure!(
            cx.update(|cx| workspace::move_between(id, 0, Some((target, side)), cx))
                .is_err(),
            "Insertion ignored the destination layout lock"
        );
        ensure!(
            serde_json::to_value(&locked)?
                == serde_json::to_value(cx.update(|cx| workspace::snapshot(cx)))?,
            "Rejected insertion changed the workspace"
        );
        cx.update_window(main_handle, |_, window, cx| {
            main_dock.update(cx, |dock, cx| dock.set_locked(false, window, cx));
        })?;
        ensure!(
            cx.update(|cx| workspace::move_between(id, 0, Some((u64::MAX - 1, side)), cx))
                .is_err(),
            "Insertion accepted a stale target"
        );
        let preserved = cx.update(|cx| workspace::snapshot(cx));
        let (owner, still_floating) = unique_panel(&preserved, id)?;
        ensure!(
            owner == floating.id
                && still_floating.model == added_panel.model
                && still_floating.content == added_panel.content,
            "A failed insertion lost panel state"
        );

        cx.update(|cx| workspace::move_between(id, 0, Some((target, side)), cx))?;
        cx.run_until_parked();
        render_workspace_windows(cx)?;
        let inserted = cx.update(|cx| workspace::snapshot(cx));
        storage::validate(&inserted)?;
        let (owner, moved) = unique_panel(&inserted, id)?;
        ensure!(
            owner == 0 && moved.model == added_panel.model && moved.content == added_panel.content,
            "Insertion changed component identity or content"
        );
        ensure!(
            inserted.windows.len() == original.windows.len(),
            "The empty floating window was not closed"
        );
        let center = &inserted
            .windows
            .iter()
            .find(|w| w.id == 0)
            .context("Main window missing")?
            .dock
            .center;
        ensure!(
            placement_matches(center, id, target, side)?,
            "Component was not inserted on the requested side: {side:?}"
        );
        cx.update(workspace::verify_workspace_persistence)?;
        let restored = cx.update(|cx| workspace::snapshot(cx));
        let (owner, restored_panel) = unique_panel(&restored, id)?;
        ensure!(
            owner == 0
                && restored_panel.model == added_panel.model
                && restored_panel.content == added_panel.content,
            "Inserted component did not survive restore/undo"
        );
        cx.update(|cx| workspace::restore(original.clone(), cx))?;
        cx.run_until_parked();
        render_workspace_windows(cx)?;
    }
    println!(
        "PASS: new components open separate native windows; exact five-way reinsertion, locks, stale targets, state retention and restore/undo"
    );
    Ok(())
}

fn render_workspace_windows(cx: &mut HeadlessAppContext) -> Result<()> {
    let state = cx.update(|cx| workspace::snapshot(cx));
    for saved in state.windows {
        let handle = cx
            .update(|cx| workspace::workspace(saved.id, cx).map(|w| w.read(cx).handle))
            .context("Workspace has no native window")?;
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    }
    cx.run_until_parked();
    Ok(())
}

fn placement_matches(state: &PanelState, moved: u64, target: u64, side: DockSide) -> Result<bool> {
    let mut moved_child = None;
    let mut target_child = None;
    for (index, child) in state.children.iter().enumerate() {
        let mut panels = Vec::new();
        collect_panels(child, &mut panels)?;
        if panels.iter().any(|p| p.model.id == moved) {
            moved_child = Some(index);
        }
        if panels.iter().any(|p| p.model.id == target) {
            target_child = Some(index);
        }
    }
    let (Some(moved_child), Some(target_child)) = (moved_child, target_child) else {
        return Ok(false);
    };
    if moved_child == target_child {
        return placement_matches(&state.children[moved_child], moved, target, side);
    }
    Ok(match (&state.info, side) {
        (PanelInfo::Tabs { active_index }, DockSide::Center) => *active_index == moved_child,
        (PanelInfo::Stack { axis: 0, .. }, DockSide::Left) => moved_child < target_child,
        (PanelInfo::Stack { axis: 0, .. }, DockSide::Right) => moved_child > target_child,
        (PanelInfo::Stack { axis: 1, .. }, DockSide::Top) => moved_child < target_child,
        (PanelInfo::Stack { axis: 1, .. }, DockSide::Bottom) => moved_child > target_child,
        _ => false,
    })
}

fn verify_detach_roundtrip(cx: &mut HeadlessAppContext, expected: &SavedPanel) -> Result<()> {
    let before = cx.update(|cx| workspace::snapshot(cx));
    let (original_window, _) = unique_panel(&before, expected.model.id)?;
    ensure!(
        original_window == 0,
        "The initial chart is not in the main workspace"
    );

    cx.update(|cx| workspace::detach(expected.model.id, None, cx))?;
    cx.run_until_parked();
    let detached = cx.update(|cx| workspace::snapshot(cx));
    storage::validate(&detached)?;
    ensure!(
        detached.windows.len() == before.windows.len() + 1,
        "Detaching did not create an independent workspace"
    );
    let (secondary, moved) = unique_panel(&detached, expected.model.id)?;
    ensure!(
        secondary != original_window,
        "The detached chart stayed in its original workspace"
    );
    ensure!(
        moved.model == expected.model && moved.content == expected.content,
        "Detaching changed the chart identity, symbol, period, link group or viewport"
    );
    let secondary_handle = cx
        .update(|cx| workspace::workspace(secondary, cx).map(|w| w.read(cx).handle))
        .context("The independent workspace has no native window")?;
    cx.update_window(secondary_handle, |_, window, cx| window.render_frame(cx))?;
    cx.update(workspace::verify_transfer_geometry)?;
    let detached_pixels = cx
        .capture_screenshot(secondary_handle)
        .context("The detached chart did not render in its native window")?;
    ensure!(
        detached_pixels
            .pixels()
            .filter(|p| p[0].abs_diff(p[1]) > 30)
            .count()
            > 100,
        "The detached chart window contains no rendered market data"
    );

    // Exercise production restoration and undo while a second window exists.
    cx.update(workspace::verify_workspace_persistence)?;
    cx.run_until_parked();
    let restored = cx.update(|cx| workspace::snapshot(cx));
    let (restored_window, restored_panel) = unique_panel(&restored, expected.model.id)?;
    ensure!(
        restored_window == secondary
            && restored_panel.model == expected.model
            && restored_panel.content == expected.content,
        "Restore/undo did not retain the independent chart state"
    );

    // Explicitly returning a component preserves it; closing its native window
    // instead deletes it, as exercised by verify_floating_window_close.
    cx.update(|cx| workspace::return_window(secondary, cx))?;
    cx.run_until_parked();
    let returned = cx.update(|cx| workspace::snapshot(cx));
    storage::validate(&returned)?;
    ensure!(
        returned.windows.len() == before.windows.len()
            && returned.windows.iter().all(|window| window.id != secondary),
        "Returning the panel did not close its empty independent window"
    );
    let (owner, returned_panel) = unique_panel(&returned, expected.model.id)?;
    ensure!(
        owner == original_window
            && returned_panel.model == expected.model
            && returned_panel.content == expected.content,
        "Returning the chart changed its durable identity or content state"
    );
    Ok(())
}

/// The headless platform does not dispatch native should-close callbacks. Call
/// their production dismissal path and inspect live ownership, persistence and
/// undo; native window-manager delivery is verified separately on a desktop.
fn verify_floating_window_close(cx: &mut HeadlessAppContext) -> Result<()> {
    let original = cx.update(|cx| workspace::snapshot_board_set(cx));
    let result = verify_window_dismissal(cx);
    let restored = cx.update(|cx| workspace::restore_board_set(original, cx));
    cx.run_until_parked();
    result?;
    restored?;
    render_workspace_windows(cx)?;
    println!(
        "PASS: closing single/multi-tab floating windows deletes their components despite layout locks; no registry or persisted resurrection, content-preserving undo and inactive-board isolation"
    );
    Ok(())
}

fn verify_window_dismissal(cx: &mut HeadlessAppContext) -> Result<()> {
    let active = cx.update(|cx| workspace::active_board_id(cx));
    let initial = cx.update(|cx| workspace::snapshot(cx));
    let chart = board_panels(&initial)?
        .into_iter()
        .find(|(_, p)| p.model.kind == PanelKind::Chart)
        .context("No chart available for close/undo regression")?
        .1;
    ensure!(
        chart.model.symbol == Symbol::Msft
            && chart.model.period == Period::Day
            && chart.content["chart"]["bars_visible"].as_u64().is_some(),
        "Close regression requires the preceding native symbol, period and zoom edits"
    );
    let (entry_owner, entry) = add_floating_panel(cx, PanelKind::OrderEntry)?;

    // The inactive board has its own copied floating form; deletion must affect
    // only the currently mounted instance, never another board's saved windows.
    let inactive = cx.update(|cx| workspace::duplicate_board("关闭隔离验收", cx))?;
    cx.update(|cx| workspace::switch_board(active, cx))?;
    render_workspace_windows(cx)?;
    set_workspace_locked(cx, 0, true)?;

    // Edit after the most recent board/layout record. Dismissal must capture
    // these current inputs itself instead of depending on an autosave tick.
    let entry_handle = native_workspace(cx, entry_owner)?;
    cx.update_window(entry_handle, |_, window, cx| {
        window.click("order-price", cx);
        window.press("ctrl-a", cx);
        window.input("427.65", cx);
        window.click("order-quantity", cx);
        window.press("ctrl-a", cx);
        window.input("375", cx);
    })?;
    render_workspace_windows(cx)?;
    let entry_state = unique_panel(&cx.update(|cx| workspace::snapshot(cx)), entry.model.id)?.1;
    ensure!(
        entry_state.content["price"] == "427.65" && entry_state.content["quantity"] == "375",
        "Native order input did not enter the close regression snapshot"
    );
    let before_single = dismiss_and_verify(cx, entry_owner)?;
    cx.update(workspace::undo);
    render_workspace_windows(cx)?;
    let restored_single = cx.update(|cx| workspace::snapshot(cx));
    ensure!(
        same_board_workspace(&before_single, &restored_single)?
            && unique_panel(&restored_single, entry.model.id)?.1.content == entry_state.content,
        "Undo failed to recover the closed independent form and its current input"
    );

    // Assemble two components as tabs: the chart becomes inactive, so a close
    // implementation that only removes the active form must fail this test.
    set_workspace_locked(cx, 0, false)?;
    cx.update(|cx| workspace::detach(chart.model.id, None, cx))?;
    let chart_owner = unique_panel(&cx.update(|cx| workspace::snapshot(cx)), chart.model.id)?.0;
    cx.update(|cx| {
        workspace::move_between(
            entry.model.id,
            chart_owner,
            Some((chart.model.id, DockSide::Center)),
            cx,
        )
    })?;
    let (survivor, _) = add_floating_panel(cx, PanelKind::Quote)?;
    set_workspace_locked(cx, 0, true)?;
    set_workspace_locked(cx, chart_owner, true)?;
    render_workspace_windows(cx)?;
    let multi = cx.update(|cx| workspace::snapshot(cx));
    let center = &multi
        .windows
        .iter()
        .find(|window| window.id == chart_owner)
        .context("Multi-tab close fixture has no floating window")?
        .dock
        .center;
    ensure!(
        placement_matches(center, entry.model.id, chart.model.id, DockSide::Center)?
            && board_panels(&multi)?
                .iter()
                .filter(|(owner, _)| *owner == chart_owner)
                .count()
                == 2,
        "Close regression did not create both active and inactive floating tabs"
    );
    let before_multi = dismiss_and_verify(cx, chart_owner)?;
    ensure!(
        cx.update(|cx| workspace::workspace(survivor, cx).is_some()),
        "Closing one floating window removed a different native window"
    );
    cx.update(workspace::undo);
    render_workspace_windows(cx)?;
    let restored_multi = cx.update(|cx| workspace::snapshot(cx));
    let (_, restored_chart) = unique_panel(&restored_multi, chart.model.id)?;
    let (_, restored_entry) = unique_panel(&restored_multi, entry.model.id)?;
    ensure!(
        same_board_workspace(&before_multi, &restored_multi)?
            && restored_chart.model == chart.model
            && restored_chart.content == chart.content
            && restored_entry.model == entry_state.model
            && restored_entry.content == entry_state.content,
        "Undo failed to recover all closed tabs with their symbol, form and viewport"
    );

    dismiss_and_verify(cx, chart_owner)?;
    let deleted = cx.update(|cx| workspace::snapshot_board_set(cx));
    let directory = BoardSmokeDirectory::new()?;
    let path = directory.0.join("workspace.json");
    storage::write_board_set(&path, &deleted)?;
    let loaded = storage::read_board_set(&path)?;
    ensure!(
        serde_json::to_value(&deleted)? == serde_json::to_value(&loaded)?,
        "Saving a component deletion changed other board content"
    );
    cx.update(|cx| workspace::restore_board_set(loaded, cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        same_board_workspace(
            &deleted
                .boards
                .iter()
                .find(|b| b.id == active)
                .unwrap()
                .workspace,
            &cx.update(|cx| workspace::snapshot(cx)),
        )?,
        "Reload resurrected deleted floating components"
    );
    cx.update(|cx| workspace::switch_board(inactive, cx))?;
    render_workspace_windows(cx)?;
    ensure!(
        same_board_workspace(
            &deleted
                .boards
                .iter()
                .find(|b| b.id == inactive)
                .unwrap()
                .workspace,
            &cx.update(|cx| workspace::snapshot(cx)),
        )?,
        "Deleting active-board components changed an inactive board's form or window"
    );
    cx.update(|cx| workspace::switch_board(active, cx))?;
    render_workspace_windows(cx)?;
    let final_state = cx.update(|cx| workspace::snapshot(cx));
    ensure!(
        board_panels(&final_state)?.iter().all(|(_, panel)| {
            panel.model.id != chart.model.id && panel.model.id != entry.model.id
        }) && cx
            .update(|cx| workspace::live_panel_ids(cx))
            .iter()
            .all(|id| { *id != chart.model.id && *id != entry.model.id }),
        "Switching boards resurrected deleted components in layout or live ownership"
    );
    Ok(())
}

fn add_floating_panel(cx: &mut HeadlessAppContext, kind: PanelKind) -> Result<(u64, SavedPanel)> {
    let before = cx.update(|cx| workspace::snapshot(cx));
    cx.update(|cx| workspace::add_panel(0, kind, cx))?;
    render_workspace_windows(cx)?;
    board_panels(&cx.update(|cx| workspace::snapshot(cx)))?
        .into_iter()
        .find(|(owner, panel)| {
            panel.model.kind == kind && !before.windows.iter().any(|w| w.id == *owner)
        })
        .context("Adding a close-regression component did not create its own window")
}

fn set_workspace_locked(cx: &mut HeadlessAppContext, id: u64, locked: bool) -> Result<()> {
    let view = cx
        .update(|cx| workspace::workspace(id, cx))
        .context("Cannot lock missing window")?;
    let (handle, dock) = cx.update(|cx| (view.read(cx).handle, view.read(cx).dock.clone()));
    cx.update_window(handle, |_, window, cx| {
        dock.update(cx, |dock, cx| dock.set_locked(locked, window, cx));
    })?;
    render_workspace_windows(cx)
}

fn dismiss_and_verify(cx: &mut HeadlessAppContext, id: u64) -> Result<WorkspaceFile> {
    let before = cx.update(|cx| workspace::snapshot(cx));
    let boards = cx.update(|cx| workspace::snapshot_board_set(cx));
    let handle = native_workspace(cx, id)?;
    cx.update(|cx| workspace::dismiss_window(id, cx))?;
    render_workspace_windows(cx)?;
    let after = cx.update(|cx| workspace::snapshot(cx));
    storage::validate(&after)?;
    let mut expected = before.clone();
    expected.windows.retain(|window| window.id != id);
    ensure!(
        same_board_workspace(&expected, &after)?
            && cx.update(|cx| workspace::workspace(id, cx).is_none())
            && cx.update_window(handle, |_, _, _| ()).is_err(),
        "Closing a floating window returned components to the main window or changed another window"
    );
    let mut expected_ids: Vec<_> = board_panels(&expected)?
        .into_iter()
        .map(|(_, panel)| panel.model.id)
        .collect();
    expected_ids.sort_unstable();
    let mut live_ids = cx.update(|cx| workspace::live_panel_ids(cx));
    live_ids.sort_unstable();
    ensure!(
        expected_ids == live_ids,
        "Closing a floating window left stale component ownership or removed unrelated components"
    );
    let after_boards = cx.update(|cx| workspace::snapshot_board_set(cx));
    for board in boards.boards.iter().filter(|b| b.id != boards.active_board) {
        let preserved = after_boards.boards.iter().find(|b| b.id == board.id);
        ensure!(
            preserved.is_some_and(
                |after| serde_json::to_value(after).ok() == serde_json::to_value(board).ok()
            ),
            "Closing a floating window changed an inactive board snapshot"
        );
    }
    Ok(before)
}

fn unique_panel(state: &WorkspaceFile, id: u64) -> Result<(u64, SavedPanel)> {
    let mut matches = Vec::new();
    for window in &state.windows {
        let mut panels = Vec::new();
        collect_panels(&window.dock.center, &mut panels)?;
        matches.extend(
            panels
                .into_iter()
                .filter(|panel| panel.model.id == id)
                .map(|panel| (window.id, panel)),
        );
    }
    ensure!(
        matches.len() == 1,
        "Expected exactly one copy of panel {id}; found {}",
        matches.len()
    );
    Ok(matches.pop().expect("exactly one saved panel"))
}

fn collect_panels(state: &PanelState, result: &mut Vec<SavedPanel>) -> Result<()> {
    if state.panel_name == "tydaro.panel" {
        let PanelInfo::Panel(value) = &state.info else {
            anyhow::bail!("A Tydaro panel has no serialized model");
        };
        result.push(serde_json::from_value(value.clone()).context("Invalid live panel snapshot")?);
    }
    for child in &state.children {
        collect_panels(child, result)?;
    }
    Ok(())
}
