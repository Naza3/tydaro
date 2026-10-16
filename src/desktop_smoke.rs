//! Native interactions for the unified five-page desktop, using the production
//! pages and business state. This module is compiled only with `test-support`.
use std::path::Path;

use anyhow::{Context as _, Result, ensure};
use gpui_kit::{
    AnyWindowHandle, Entity, HeadlessAppContext, ScrollDelta,
    component::dock::{PanelInfo, PanelState},
    point, px, size,
    test::TestWindowExt,
};

use crate::{
    desktop_state::{DEMO_DATE, DesktopState, PageId},
    market::Symbol,
    model::PanelKind,
    storage::SavedPanel,
    workspace::Workspace,
};

/// Exercise the same native elements used by desktop users. The caller passes
/// the current workspace after legacy board tests have restored their windows.
pub(crate) fn verify(
    cx: &mut HeadlessAppContext,
    workspace: &Entity<Workspace>,
    output: &Path,
) -> Result<()> {
    let handle = cx.update(|cx| workspace.read(cx).handle);
    click(cx, handle, "desktop-nav-market")?;
    resize(cx, handle, 1536, 960)?;
    verify_search(cx, handle, workspace)?;
    let market_before = cx.update(|cx| workspace.read(cx).dock.read(cx).dump(cx));
    capture_pages(cx, handle, output, 1536, 960)?;
    click(cx, handle, "desktop-nav-market")?;
    let market_after = cx.update(|cx| workspace.read(cx).dock.read(cx).dump(cx));
    ensure!(
        market_before == market_after,
        "Switching business pages changed market panel identities, selections or content"
    );

    verify_watch(cx, handle)?;
    verify_research_handoff(cx, handle)?;
    verify_journal(cx, handle, output)?;

    capture_pages(cx, handle, output, 1280, 800)?;
    click(cx, handle, "desktop-nav-market")?;
    resize(cx, handle, 1536, 960)?;
    println!(
        "PASS: five native desktop pages at 1536 × 960 and 1280 × 800; market identity preservation; watch groups and global monitoring; journal validation, sell-only plans, ID updates, independent drafts and native reading-window close"
    );
    Ok(())
}

fn verify_search(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    workspace: &Entity<Workspace>,
) -> Result<()> {
    click(cx, handle, "native-market-chart")?;
    cx.update_window(handle, |_, window, cx| window.press("ctrl-k", cx))?;
    render(cx, handle)?;
    input(cx, handle, "desktop-search-input", "603598")?;
    click(cx, handle, "desktop-search-result-603598")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().active_page) == PageId::Market,
        "Global stock search did not open the market page"
    );
    let dock = cx.update(|cx| workspace.read(cx).dock.read(cx).dump(cx));
    let mut panels = Vec::new();
    collect_panels(&dock.center, &mut panels)?;
    ensure!(
        panels
            .iter()
            .any(|p| p.model.kind == PanelKind::Chart && p.model.symbol == Symbol::Yinli),
        "Global stock search did not select its stock in the production chart"
    );
    click(cx, handle, "desktop-search-trigger")?;
    input(cx, handle, "desktop-search-input", "复盘")?;
    click(cx, handle, "desktop-search-page-journal")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().active_page) == PageId::Journal,
        "Global page search did not navigate to the requested page"
    );
    click(cx, handle, "desktop-nav-market")?;
    println!(
        "PASS: native Ctrl+K stock search selects the production chart; page search navigates to the requested business page"
    );
    Ok(())
}

fn collect_panels(state: &PanelState, panels: &mut Vec<SavedPanel>) -> Result<()> {
    if state.panel_name == "tydaro.panel" {
        let PanelInfo::Panel(value) = &state.info else {
            anyhow::bail!("A native market panel has no saved business model");
        };
        panels.push(serde_json::from_value(value.clone())?);
    }
    for child in &state.children {
        collect_panels(child, panels)?;
    }
    Ok(())
}

fn verify_research_handoff(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> Result<()> {
    click(cx, handle, "desktop-nav-limit")?;
    input(cx, handle, "limit-filter", "AI")?;
    click(cx, handle, "limit-filter-add")?;
    input(cx, handle, "limit-filter", DEMO_DATE)?;
    click(cx, handle, "limit-filter-add")?;
    cx.update_window(handle, |_, window, _| {
        ensure!(
            window
                .try_find("limit-inspect-2026-10-14-AI 应用-603598")
                .is_some()
                && window
                    .try_find("limit-inspect-2026-10-13-AI 应用-603598")
                    .is_none()
                && window
                    .try_find("limit-inspect-2026-10-14-汽车产业链-601127")
                    .is_none(),
            "Research filters did not intersect the sector and date conditions"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    click(cx, handle, "limit-filter-remove-1")?;
    cx.update_window(handle, |_, window, _| {
        ensure!(
            window
                .try_find("limit-inspect-2026-10-13-AI 应用-603598")
                .is_some(),
            "Removing one AND condition kept stale search results"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    click(cx, handle, "limit-filter-clear")?;

    let before = cx.update(|cx| cx.global::<DesktopState>().watch_groups.clone());
    click(cx, handle, "limit-add-watch")?;
    ensure!(
        cx.update(|cx| serde_json::to_value(&cx.global::<DesktopState>().watch_groups))?
            == serde_json::to_value(&before)?,
        "Opening the handoff dialog modified watch groups before confirmation"
    );
    click(cx, handle, "watch-target-media")?;
    click(cx, handle, "watch-add-confirm")?;
    cx.update(|cx| {
        let groups = &cx.global::<DesktopState>().watch_groups;
        let media = groups
            .iter()
            .find(|g| g.id == "media")
            .context("The media group disappeared")?;
        let old = before
            .iter()
            .find(|g| g.id == "media")
            .context("Expected the seeded media group")?;
        ensure!(
            media.symbols.len() == old.symbols.len() + 1
                && media.symbols.contains(&Symbol::Luxshare),
            "Research handoff did not append the new stock while deduplicating existing stocks"
        );
        for group in before.iter().filter(|g| g.id != "media") {
            ensure!(
                groups
                    .iter()
                    .find(|g| g.id == group.id)
                    .is_some_and(|current| current.symbols == group.symbols),
                "Research handoff modified a non-target group"
            );
        }
        Ok::<_, anyhow::Error>(())
    })?;
    click(cx, handle, "desktop-nav-limit")?;
    click(cx, handle, "limit-add-watch")?;
    click(cx, handle, "watch-target-media")?;
    click(cx, handle, "watch-add-confirm")?;
    cx.update_window(handle, |_, window, _| {
        ensure!(
            window.try_find("watch-add-cancel").is_some(),
            "An entirely duplicate handoff incorrectly committed an append action"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    click(cx, handle, "watch-add-cancel")?;
    ensure!(
        cx.update(|cx| cx
            .global::<DesktopState>()
            .watch_groups
            .iter()
            .find(|g| g.id == "media")
            .map(|g| g.symbols.len()))
            == before
                .iter()
                .find(|g| g.id == "media")
                .map(|g| g.symbols.len() + 1),
        "Repeated explicit handoff duplicated existing group members"
    );

    click(cx, handle, "desktop-nav-limit")?;
    click(cx, handle, "limit-open-theme")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().active_page) == PageId::Theme,
        "Research context did not navigate to the native theme page"
    );
    input(cx, handle, "theme-cutoff-date", "2026-10-15")?;
    let journal_before =
        cx.update(|cx| serde_json::to_value(&cx.global::<DesktopState>().journal))?;
    click(cx, handle, "theme-create-draft")?;
    ensure!(
        cx.update(|cx| serde_json::to_value(&cx.global::<DesktopState>().journal))?
            == journal_before,
        "Opening the draft dialog wrote a journal draft before confirmation"
    );
    input(cx, handle, "draft-business-date", "2026-10-16")?;
    click(cx, handle, "draft-create-confirm")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().active_page) == PageId::Journal,
        "Confirming a research draft did not open its editor"
    );
    cx.update(|cx| {
        let day = cx
            .global::<DesktopState>()
            .journal
            .days
            .get("2026-10-16")
            .context("The selected journal business date was ignored")?;
        let draft = day
            .drafts
            .iter()
            .find(|draft| draft.plan.sector == "AI 应用")
            .context("Theme handoff did not create a sourced draft")?;
        ensure!(
            draft.plan.date == "2026-10-16"
                && draft.plan.source.contains(DEMO_DATE)
                && draft.plan.source.contains("题材"),
            "Source actual date and target business date were conflated"
        );
        ensure!(
            draft.plan.symbols == vec![Symbol::Yinli]
                && draft.plan.method.is_empty()
                && draft.plan.entry.is_empty(),
            "Research handoff changed its stock or fabricated a completed trading plan"
        );
        Ok::<_, anyhow::Error>(())
    })?;
    click(cx, handle, "journal-draft-save")?;
    println!(
        "PASS: research AND filters and removal; confirmed target-only watch append with duplicate rejection; theme source date remains distinct from the journal business date"
    );
    Ok(())
}

fn verify_watch(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> Result<()> {
    click(cx, handle, "desktop-nav-watch")?;
    let original = cx.update(|cx| cx.global::<DesktopState>().watch_groups.clone());
    click(cx, handle, "watch-add-group")?;
    input(cx, handle, "watch-group-name", "原生回归观察")?;
    input(cx, handle, "watch-group-note", "成员与其他分组保持独立")?;
    click(cx, handle, "watch-member-603598")?;
    click(cx, handle, "watch-group-save")?;
    let group = cx
        .update(|cx| {
            cx.global::<DesktopState>()
                .watch_groups
                .iter()
                .find(|group| group.name == "原生回归观察")
                .cloned()
        })
        .context("Saving a native group form did not add its group")?;
    ensure!(
        group.symbols == vec![Symbol::Yinli],
        "The new watch group did not preserve the selected members"
    );
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().watch_groups.len()) == original.len() + 1,
        "New group creation changed an existing group instead of creating an independent group"
    );
    click(cx, handle, &format!("watch-nav-{}", group.id))?;
    click(cx, handle, &format!("watch-group-edit-{}", group.id))?;
    input(cx, handle, "watch-group-name", "原生回归 · 已改名")?;
    click(cx, handle, "watch-group-save")?;
    cx.update(|cx| {
        let groups = &cx.global::<DesktopState>().watch_groups;
        ensure!(
            groups
                .iter()
                .any(|item| item.id == group.id && item.name == "原生回归 · 已改名"),
            "Renaming a group changed its identity or did not save"
        );
        for old in &original {
            let current = groups
                .iter()
                .find(|item| item.id == old.id)
                .context("Another watch group was deleted")?;
            ensure!(
                current.name == old.name && current.symbols == old.symbols,
                "Editing one watch group changed another group"
            );
        }
        Ok::<_, anyhow::Error>(())
    })?;

    let monitoring = cx.update(|cx| cx.global::<DesktopState>().monitoring);
    click(cx, handle, "watch-monitor")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().monitoring) != monitoring,
        "Native monitoring toggle did not update shared state"
    );
    click(cx, handle, "desktop-nav-theme")?;
    click(cx, handle, "desktop-nav-watch")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().monitoring) != monitoring,
        "Changing pages reset global monitoring"
    );
    if !cx.update(|cx| cx.global::<DesktopState>().monitoring) {
        click(cx, handle, "watch-monitor")?;
    }
    click(cx, handle, "watch-show-alerts")?;
    let alerts_before = cx.update(|cx| cx.global::<DesktopState>().alerts.len());
    click(cx, handle, "watch-demo-alert")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().alerts.len()) == alerts_before + 1,
        "Explicit demo alert did not append one event"
    );
    click(cx, handle, "watch-alerts-read")?;
    ensure!(
        cx.update(|cx| {
            let s = cx.global::<DesktopState>();
            s.unread_count() == 0 && s.alerts.len() == alerts_before + 1
        }),
        "Marking alerts read discarded their records or left unread alerts"
    );
    click(cx, handle, "watch-alerts-clear")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().alerts.is_empty()),
        "Clear alerts changed only the badge instead of removing records"
    );
    let muted = cx.update(|cx| cx.global::<DesktopState>().muted);
    click(cx, handle, "watch-muted")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().muted) != muted,
        "Mute preference did not change"
    );
    click(cx, handle, "watch-muted")?;
    println!(
        "PASS: native watch group creation/rename and member isolation; global monitoring across pages; explicit alerts, read preservation, clear-read and mute preference"
    );
    Ok(())
}

fn verify_journal(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    output: &Path,
) -> Result<()> {
    const DATE: &str = "2026-10-15";
    click(cx, handle, "desktop-nav-journal")?;
    click(cx, handle, "journal-add-hotspot")?;
    input(cx, handle, "journal-record-name", "多行观察")?;
    input(
        cx,
        handle,
        "journal-record-description",
        "首板观察\n次日确认",
    )?;
    click(cx, handle, "journal-record-save")?;
    let hotspot_id = cx
        .update(|cx| {
            cx.global::<DesktopState>()
                .journal
                .hotspots
                .iter()
                .find(|item| item.sector == "多行观察")
                .map(|item| item.id.clone())
        })
        .context("Saving a manual hotspot did not create its record")?;
    click(cx, handle, &format!("journal-hot-edit-{hotspot_id}"))?;
    input(cx, handle, "journal-record-name", "多行观察改名")?;
    click(cx, handle, "journal-record-save")?;
    ensure!(
        cx.update(|cx| cx
            .global::<DesktopState>()
            .journal
            .hotspots
            .iter()
            .any(|item| item.id == hotspot_id
                && item.sector == "多行观察改名"
                && item.heights == "首板观察\n次日确认")),
        "Editing a hotspot name normalized or discarded its multiline observation"
    );
    input(cx, handle, "journal-date-input", DATE)?;
    click(cx, handle, "journal-date-apply")?;
    let original_plans = cx.update(|cx| {
        cx.global::<DesktopState>()
            .journal
            .days
            .get(DATE)
            .map_or(0, |day| day.plans.len())
    });
    let original_drafts = cx.update(|cx| {
        cx.global::<DesktopState>()
            .journal
            .days
            .get(DATE)
            .map_or(0, |day| day.drafts.len())
    });

    click(cx, handle, "journal-new-plan")?;
    input(cx, handle, "journal-sector", "未完成的买入预案")?;
    input(cx, handle, "journal-method", "分歧转一致")?;
    click(cx, handle, "desktop-nav-watch")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().active_page) == PageId::Journal,
        "Page navigation bypassed the active native journal editor"
    );
    cx.update_window(handle, |_, window, cx| window.press("ctrl-k", cx))?;
    render(cx, handle)?;
    cx.update_window(handle, |_, window, _| {
        ensure!(
            window.try_find("desktop-search-input").is_none()
                && window.find("journal-editor").visible(),
            "Global search covered an unfinished journal editor instead of preserving its context"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    click(cx, handle, "journal-plan-save")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().journal.days[DATE].plans.len())
            == original_plans,
        "Incomplete buy side was incorrectly committed as a plan"
    );
    click(cx, handle, "journal-draft-save")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().journal.days[DATE].drafts.len())
            == original_drafts + 1,
        "Saving an incomplete draft lost the typed work"
    );

    click(cx, handle, "journal-new-plan")?;
    input(cx, handle, "journal-sector", "原生卖出预案")?;
    input(cx, handle, "journal-sell-method", "跌破支撑离场")?;
    input(cx, handle, "journal-exit", "收盘跌破二十日均线时离场")?;
    click(cx, handle, "journal-plan-save")?;
    let plan = cx
        .update(|cx| {
            cx.global::<DesktopState>().journal.days[DATE]
                .plans
                .iter()
                .find(|plan| plan.sector == "原生卖出预案")
                .cloned()
        })
        .context("A complete sell-only plan could not be saved")?;
    ensure!(
        plan.method.is_empty() && plan.entry.is_empty(),
        "Saving a sell-only plan invented a buy side"
    );
    click(cx, handle, &format!("journal-edit-{}", plan.id))?;
    input(cx, handle, "journal-exit", "跌破支撑且放量时分批离场")?;
    click(cx, handle, "journal-plan-save")?;
    cx.update(|cx| {
        let day = &cx.global::<DesktopState>().journal.days[DATE];
        ensure!(
            day.plans.len() == original_plans + 1,
            "Editing an existing plan appended a duplicate"
        );
        ensure!(
            day.plans
                .iter()
                .any(|item| item.id == plan.id && item.exit == "跌破支撑且放量时分批离场"),
            "Editing did not update the original plan identity"
        );
        Ok::<_, anyhow::Error>(())
    })?;

    click(cx, handle, "journal-new-plan")?;
    input(cx, handle, "journal-sector", "独立待补充草稿")?;
    input(cx, handle, "journal-basis", "此草稿不覆盖另一个未完成预案")?;
    click(cx, handle, "journal-draft-save")?;
    input(cx, handle, "journal-date-input", DEMO_DATE)?;
    click(cx, handle, "journal-date-apply")?;
    input(cx, handle, "journal-date-input", DATE)?;
    click(cx, handle, "journal-date-apply")?;
    cx.update(|cx| {
        let day = &cx.global::<DesktopState>().journal.days[DATE];
        ensure!(
            day.drafts.len() == original_drafts + 2,
            "Switching dates overwrote an independent draft"
        );
        ensure!(
            day.drafts
                .iter()
                .any(|draft| draft.plan.sector == "未完成的买入预案"
                    && draft.plan.method == "分歧转一致"),
            "The first draft lost its incomplete side after a second draft was created"
        );
        Ok::<_, anyhow::Error>(())
    })?;

    reveal(cx, handle, "journal-document-scroll", "journal-note")?;
    input(cx, handle, "journal-note", "主窗口笔记初稿")?;
    ensure!(
        cx.update(|cx| cx.global::<DesktopState>().journal.days[DATE].note.clone())
            == "主窗口笔记初稿",
        "Typing a main journal note did not update its business document"
    );
    let windows = cx.update(|cx| cx.windows());
    click(cx, handle, "journal-reading-open")?;
    let reading = cx
        .update(|cx| {
            cx.windows()
                .into_iter()
                .find(|window| !windows.contains(window))
        })
        .context("Reading mode did not open an independent native window")?;
    render(cx, reading)?;
    input(cx, handle, "journal-note", "主窗口更新后，阅读窗口同步显示")?;
    render(cx, reading)?;
    cx.update_window(reading, |_, window, _| {
        let wrapper = window.find("journal-reading-note");
        let values: Vec<_> = gpui_kit::base::test_support::snapshots(window).into_iter()
            .filter(|node| node.path().starts_with(wrapper.path()))
            .filter_map(|node| node.value().map(str::to_owned)).collect();
        ensure!(values == ["主窗口更新后，阅读窗口同步显示"], "The already-open reading window did not receive the main window's same-date note update: {values:?}");
        Ok::<_, anyhow::Error>(())
    })??;
    let other_note = cx.update(|cx| {
        cx.global::<DesktopState>().journal.days[DEMO_DATE]
            .note
            .clone()
    });
    input(cx, handle, "journal-date-input", DEMO_DATE)?;
    click(cx, handle, "journal-date-apply")?;
    input(
        cx,
        reading,
        "journal-reading-note",
        "独立阅读窗口只编辑固定日期的记录",
    )?;
    cx.update(|cx| {
        let days = &cx.global::<DesktopState>().journal.days;
        ensure!(
            days[DATE].note == "独立阅读窗口只编辑固定日期的记录"
                && days[DEMO_DATE].note == other_note,
            "A reading window wrote its note into the main page's subsequently selected date"
        );
        Ok::<_, anyhow::Error>(())
    })?;
    let day_before =
        cx.update(|cx| serde_json::to_value(&cx.global::<DesktopState>().journal.days[DATE]))?;
    save_screenshot(cx, reading, output, "desktop-journal-reading", None)?;
    cx.update_window(reading, |_, window, cx| {
        window.click("journal-reading-close", cx)
    })?;
    render(cx, handle)?;
    let day_after =
        cx.update(|cx| serde_json::to_value(&cx.global::<DesktopState>().journal.days[DATE]))?;
    ensure!(
        day_before == day_after,
        "Closing a reading window deleted or changed journal business records"
    );
    println!(
        "PASS: journal multiline hotspot preservation, incomplete drafts, sell-only plans, identity-preserving edits and independent dates; native reading-window edits target their fixed date and closing retains records"
    );
    Ok(())
}

fn render(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> Result<()> {
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    Ok(())
}

fn click(cx: &mut HeadlessAppContext, handle: AnyWindowHandle, id: &str) -> Result<()> {
    cx.update_window(handle, |_, window, cx| window.click(id.to_owned(), cx))?;
    render(cx, handle)
}

fn reveal(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    scroll: &str,
    target: &str,
) -> Result<()> {
    for _ in 0..20 {
        let visible = cx.update_window(handle, |_, window, cx| {
            let viewport = window.find(scroll.to_owned()).bounds();
            let node = window.try_find(target.to_owned());
            if node
                .as_ref()
                .is_some_and(|node| node.visible() && viewport.contains(&node.bounds().center()))
            {
                return true;
            }
            let direction = if node.is_some_and(|node| node.bounds().center().y < viewport.top()) {
                4.
            } else {
                -4.
            };
            window.scroll(
                scroll.to_owned(),
                ScrollDelta::Lines(point(0., direction)),
                cx,
            );
            false
        })?;
        if visible {
            return Ok(());
        }
        render(cx, handle)?;
    }
    anyhow::bail!("Scrolling {scroll} could not reveal native input {target}")
}

fn input(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    id: &str,
    value: &str,
) -> Result<()> {
    cx.update_window(handle, |_, window, cx| {
        window.click(id.to_owned(), cx);
        window.press("ctrl-a", cx);
        window.press("backspace", cx);
        window.input(value, cx);
        let target = window.find(id.to_owned());
        let values: Vec<_> = if let Some(value) = target.value() {
            vec![value.to_string()]
        } else {
            gpui_kit::base::test_support::snapshots(window)
                .into_iter()
                .filter(|node| node.path().starts_with(target.path()))
                .filter_map(|node| node.value().map(str::to_owned))
                .collect()
        };
        ensure!(
            values.as_slice() == [value],
            "Native input {id} did not receive its text: {values:?}; wrapper={target:?}; input nodes={:?}",
            gpui_kit::base::test_support::snapshots(window).into_iter()
                .filter(|node| node.path().starts_with(target.path()) && node.value().is_some())
                .collect::<Vec<_>>()
        );
        Ok::<_, anyhow::Error>(())
    })??;
    render(cx, handle)
}

fn resize(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    width: u32,
    height: u32,
) -> Result<()> {
    cx.update_window(handle, |_, window, cx| {
        window.resize(size(px(width as f32), px(height as f32)));
        // TestPlatform's programmatic resize updates platform bounds without
        // dispatching an OS resize callback. Complete the public callback path
        // so responsive pages actually observe the requested logical viewport.
        window.bounds_changed(cx);
        ensure!(
            window.viewport_size() == size(px(width as f32), px(height as f32)),
            "Headless resize did not update the native logical viewport"
        );
        Ok::<_, anyhow::Error>(())
    })??;
    render(cx, handle)
}

fn capture_pages(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    output: &Path,
    width: u32,
    height: u32,
) -> Result<()> {
    resize(cx, handle, width, height)?;
    for page in ["market", "watch", "limit", "theme", "journal"] {
        click(cx, handle, &format!("desktop-nav-{page}"))?;
        if page == "watch" {
            click(cx, handle, "watch-nav-media")?;
        } else if matches!(page, "limit" | "theme") {
            cx.update_window(handle, |_, window, cx| {
                window.scroll(
                    format!("{page}-page"),
                    ScrollDelta::Lines(point(0., 100.)),
                    cx,
                );
            })?;
            render(cx, handle)?;
        }
        let expected = match page {
            "market" => PageId::Market,
            "watch" => PageId::Watch,
            "limit" => PageId::Limit,
            "theme" => PageId::Theme,
            _ => PageId::Journal,
        };
        ensure!(
            cx.update(|cx| cx.global::<DesktopState>().active_page) == expected,
            "Navigation {page} did not change the active business page"
        );
        cx.update_window(handle, |_, window, _| {
            for nav in ["market", "watch", "limit", "theme", "journal"] {
                let item = window.find(format!("desktop-nav-{nav}"));
                ensure!(
                    item.visible(),
                    "Desktop navigation {nav} is hidden on {page}"
                );
                ensure!(
                    item.bounds().right() <= px(width as f32)
                        && item.bounds().bottom() <= px(height as f32),
                    "Desktop navigation {nav} falls outside the {width} × {height} window"
                );
            }
            Ok::<_, anyhow::Error>(())
        })??;
        save_screenshot(
            cx,
            handle,
            output,
            &format!("desktop-{page}-{width}"),
            Some((width, height)),
        )?;
    }
    Ok(())
}

fn save_screenshot(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    output: &Path,
    suffix: &str,
    expected_size: Option<(u32, u32)>,
) -> Result<()> {
    let screenshot = cx
        .capture_screenshot(handle)
        .context("Native desktop screenshot failed")?;
    if let Some(expected) = expected_size {
        let scale = cx.update_window(handle, |_, window, _| window.scale_factor())?;
        let physical = (
            (expected.0 as f32 * scale).round() as u32,
            (expected.1 as f32 * scale).round() as u32,
        );
        ensure!(
            screenshot.dimensions() == physical,
            "Native screenshot {suffix} has {:?}, expected logical {expected:?} at {scale}× = {physical:?}",
            screenshot.dimensions()
        );
    }
    let stem = output
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("workspace");
    screenshot.save(output.with_file_name(format!("{stem}-{suffix}.png")))?;
    Ok(())
}
