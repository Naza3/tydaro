use super::*;

/// Names and stock membership belong to the component instance. In particular,
/// a linked quote selection must never replace another list's own members.
pub(super) fn verify_stock_lists(cx: &mut HeadlessAppContext, output: &Path) -> Result<()> {
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

pub(super) fn verify_stock_list_interactions(
    cx: &mut HeadlessAppContext,
    output: &Path,
) -> Result<()> {
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

pub(super) fn current_panel(cx: &mut HeadlessAppContext, id: u64) -> Result<SavedPanel> {
    Ok(unique_panel(&cx.update(|cx| workspace::snapshot(cx)), id)?.1)
}

pub(super) fn panel_window(cx: &mut HeadlessAppContext, id: u64) -> Result<AnyWindowHandle> {
    let owner = unique_panel(&cx.update(|cx| workspace::snapshot(cx)), id)?.0;
    native_workspace(cx, owner)
}

pub(super) fn list_members(panel: &SavedPanel) -> Result<Vec<Symbol>> {
    serde_json::from_value(panel.content["members"].clone())
        .context("The stock-list snapshot has no valid member list")
}

pub(super) fn remove_legacy_members(state: &mut PanelState, id: u64) -> Result<bool> {
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

pub(super) fn begin_panel_rename(cx: &mut HeadlessAppContext, id: u64) -> Result<()> {
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

pub(super) fn fill_panel_name(cx: &mut HeadlessAppContext, id: u64, name: &str) -> Result<()> {
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

pub(super) fn rename_panel(cx: &mut HeadlessAppContext, id: u64, name: &str) -> Result<()> {
    begin_panel_rename(cx, id)?;
    fill_panel_name(cx, id, name)?;
    let handle = panel_window(cx, id)?;
    cx.update_window(handle, |_, window, cx| window.press("enter", cx))?;
    render_workspace_windows(cx)
}

pub(super) fn click_list_control(
    cx: &mut HeadlessAppContext,
    id: u64,
    control: &str,
) -> Result<()> {
    let handle = panel_window(cx, id)?;
    cx.update_window(handle, |_, window, cx| {
        window
            .within(format!("stock-panel-{id}"))
            .click(control.to_owned(), cx);
    })?;
    render_workspace_windows(cx)
}

pub(super) fn set_list_input(
    cx: &mut HeadlessAppContext,
    id: u64,
    control: &str,
    text: &str,
) -> Result<()> {
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
