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

use crate::adapters::layout as storage;
use crate::market::{self, Period, Symbol};
use crate::model::PanelKind;
use crate::panels::chart::MarketChart;
use crate::workspace;
use crate::workspace::snapshot::{SavedPanel, WorkspaceFile};
use crate::workspace::transfer::DockSide;

/// Render the real default application and exercise the public UI with native
/// pointer events. The screenshot uses the real platform headless renderer;
/// missing GPU/software-renderer support is an error, never a blank fallback.
fn open_headless_workspace() -> Result<(HeadlessAppContext, Entity<workspace::Workspace>)> {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(crate::app::ui::AppAssets),
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
    crate::panels::smoke_market_panels(&mut cx)?;
    verify_floating_window_close(&mut cx)?;
    let current = cx
        .update(|cx| workspace::workspace(0, cx))
        .context("Main workspace missing")?;
    crate::test_support::desktop::verify(&mut cx, &current, Path::new(path), false)?;
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
    crate::pages::journal::smoke_rejected_inputs(cx)?;
    crate::pages::limit::verify_rejected_edits(cx, handle)?;
    crate::pages::watch::verify_rejected_edits(cx, handle)?;
    crate::app::smoke_close_confirmation(cx, handle)?;
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
    let result = crate::test_support::desktop::verify(cx, workspace, output, true);
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
    crate::panels::smoke_demo_orders(&mut cx)?;
    crate::panels::smoke_market_panels(&mut cx)?;
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

mod intraday;
use intraday::*;
mod imports;
use imports::*;
mod charts;
use charts::*;
mod stock_lists;
use stock_lists::*;
mod boards;
use boards::*;
mod windows;
use windows::*;
