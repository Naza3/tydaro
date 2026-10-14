//! Native, offline UI smoke validation, enabled only by `test-support`.
use std::{path::Path, sync::Arc};

use anyhow::{Context, Result, ensure};
use gpui_kit::{
    HeadlessAppContext, ScrollDelta,
    component::dock::{PanelInfo, PanelState},
    point,
    test::TestWindowExt,
};

use crate::{
    market::{self, Period, Symbol},
    model::PanelKind,
    storage::{self, SavedPanel, WorkspaceFile},
    workspace,
};

/// Render the real default application and exercise the public UI with native
/// pointer events. The screenshot uses the real platform headless renderer;
/// missing GPU/software-renderer support is an error, never a blank fallback.
pub fn run(path: &str) -> Result<()> {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| workspace::init(cx, false));
    let workspace = cx.update(|cx| workspace::open_workspace(0, None, cx))?;
    let handle = cx.update(|cx| workspace.read(cx).handle);
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
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
    crate::contents::smoke_demo_orders(&mut cx)?;

    println!(
        "PASS: native workspace {} × {}, {} panels; MSFT link group, 日 K, chart zoom; detach/return state, persistence/undo and demo order interactions. Screenshot: {}",
        screenshot.width(),
        screenshot.height(),
        initial_panels.len(),
        output.display()
    );
    Ok(())
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

    // This is the same return path invoked by closing a secondary window.
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
