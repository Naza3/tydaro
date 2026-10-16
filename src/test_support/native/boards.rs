use super::*;

/// Keep multiple complete workspaces alive across switches, including their
/// native floating windows, content and independent layout undo histories.
pub(super) fn verify_boards(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
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

pub(super) fn verify_board_interactions(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
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

pub(super) fn native_workspace(cx: &mut HeadlessAppContext, id: u64) -> Result<AnyWindowHandle> {
    cx.update(|cx| workspace::workspace(id, cx).map(|w| w.read(cx).handle))
        .context("Board has no matching native workspace window")
}

pub(super) fn begin_empty_board(cx: &mut HeadlessAppContext) -> Result<()> {
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

pub(super) fn click_board_tab(cx: &mut HeadlessAppContext, id: u64) -> Result<()> {
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

pub(super) fn save_board_screenshot(
    cx: &mut HeadlessAppContext,
    output: &Path,
    suffix: &str,
) -> Result<()> {
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

pub(super) fn board_panels(state: &WorkspaceFile) -> Result<Vec<(u64, SavedPanel)>> {
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

pub(super) fn same_board_workspace(a: &WorkspaceFile, b: &WorkspaceFile) -> Result<bool> {
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

pub(super) struct BoardSmokeDirectory(pub(super) PathBuf);

impl BoardSmokeDirectory {
    pub(super) fn new() -> Result<Self> {
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
