use super::*;

/// Exercise the same production add/insert paths as the component menu. Each
/// insertion starts from the same workspace so a split cannot pass merely
/// because an unrelated earlier split happens to have the expected axis.
pub(super) fn verify_new_floating_panels(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
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
        crate::workspace::snapshot::validate(&added)?;
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
        crate::workspace::snapshot::validate(&inserted)?;
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

pub(super) fn render_workspace_windows(cx: &mut HeadlessAppContext) -> Result<()> {
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

pub(super) fn placement_matches(
    state: &PanelState,
    moved: u64,
    target: u64,
    side: DockSide,
) -> Result<bool> {
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

pub(super) fn verify_detach_roundtrip(
    cx: &mut HeadlessAppContext,
    expected: &SavedPanel,
) -> Result<()> {
    let before = cx.update(|cx| workspace::snapshot(cx));
    let (original_window, _) = unique_panel(&before, expected.model.id)?;
    ensure!(
        original_window == 0,
        "The initial chart is not in the main workspace"
    );

    cx.update(|cx| workspace::detach(expected.model.id, None, cx))?;
    cx.run_until_parked();
    let detached = cx.update(|cx| workspace::snapshot(cx));
    crate::workspace::snapshot::validate(&detached)?;
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
    crate::workspace::snapshot::validate(&returned)?;
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
pub(super) fn verify_floating_window_close(cx: &mut HeadlessAppContext) -> Result<()> {
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

pub(super) fn verify_window_dismissal(cx: &mut HeadlessAppContext) -> Result<()> {
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

pub(super) fn add_floating_panel(
    cx: &mut HeadlessAppContext,
    kind: PanelKind,
) -> Result<(u64, SavedPanel)> {
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

pub(super) fn set_workspace_locked(
    cx: &mut HeadlessAppContext,
    id: u64,
    locked: bool,
) -> Result<()> {
    let view = cx
        .update(|cx| workspace::workspace(id, cx))
        .context("Cannot lock missing window")?;
    let (handle, dock) = cx.update(|cx| (view.read(cx).handle, view.read(cx).dock.clone()));
    cx.update_window(handle, |_, window, cx| {
        dock.update(cx, |dock, cx| dock.set_locked(locked, window, cx));
    })?;
    render_workspace_windows(cx)
}

pub(super) fn dismiss_and_verify(cx: &mut HeadlessAppContext, id: u64) -> Result<WorkspaceFile> {
    let before = cx.update(|cx| workspace::snapshot(cx));
    let boards = cx.update(|cx| workspace::snapshot_board_set(cx));
    let handle = native_workspace(cx, id)?;
    cx.update(|cx| workspace::dismiss_window(id, cx))?;
    render_workspace_windows(cx)?;
    let after = cx.update(|cx| workspace::snapshot(cx));
    crate::workspace::snapshot::validate(&after)?;
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

pub(super) fn unique_panel(state: &WorkspaceFile, id: u64) -> Result<(u64, SavedPanel)> {
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

pub(super) fn collect_panels(state: &PanelState, result: &mut Vec<SavedPanel>) -> Result<()> {
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
