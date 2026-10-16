use super::*;

/// Previewing pasted text must be read-only until confirmation. Imported
/// membership belongs to one component, even when its quote is linked.
pub(super) fn verify_stock_import(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
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

pub(super) fn verify_stock_import_interactions(
    cx: &mut HeadlessAppContext,
    output: &Path,
) -> Result<()> {
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

pub(super) fn paste_stock_import(cx: &mut HeadlessAppContext, id: u64, text: &str) -> Result<()> {
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

pub(super) fn stock_import_value(cx: &mut HeadlessAppContext, id: u64) -> Result<String> {
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

pub(super) fn expect_import_summary(
    cx: &mut HeadlessAppContext,
    id: u64,
    expected: &str,
) -> Result<()> {
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
