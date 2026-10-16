//! Live snapshot capture, layout-only undo, restore rollback and save scheduling.
use super::{
    Session, changed, close_window, default_layout, empty_tabs, keep_main_geometry, mark_dirty,
    open_workspace, refresh_market, remap_workspace_ids, report, snapshot_board_set, status,
    window_bounds, workspace,
};
use super::{
    snapshot::{SavedPanel, SavedWindow, WorkspaceFile},
    transfer::DragCoordinator,
};
use crate::adapters::layout as storage;
use crate::adapters::layout::writer as layout_persistence;
use gpui_kit::{component::dock::*, *};
use std::collections::HashMap;

pub(crate) fn snapshot(cx: &App) -> WorkspaceFile {
    let session = cx.global::<Session>();
    WorkspaceFile {
        version: crate::workspace::snapshot::SCHEMA_VERSION,
        next_id: session.next_id,
        paused: session.paused,
        windows: session
            .windows
            .values()
            .filter_map(WeakEntity::upgrade)
            .map(|view| {
                let view = view.read(cx);
                let dock = view.dock.read(cx);
                SavedWindow {
                    id: view.id,
                    bounds: view.bounds,
                    locked: dock.is_locked(),
                    dock: dock.dump(cx),
                }
            })
            .collect(),
    }
}

// Undo concerns the workspace arrangement. Market updates, search/form inputs,
// chart viewports, selected stocks and native window moves remain independently
// persisted, but must not fill the layout history once every refresh.
pub(super) fn layout_key(state: &WorkspaceFile, ignore_sizes: bool) -> serde_json::Value {
    fn normalize(panel: &mut PanelState, ignore_sizes: bool) {
        if panel.panel_name == "tydaro.panel"
            && let PanelInfo::Panel(value) = &mut panel.info
            && let Ok(saved) = serde_json::from_value::<SavedPanel>(value.clone())
        {
            *value = serde_json::json!({
                "id": saved.model.id, "kind": saved.model.kind,
                "link_group": saved.model.link_group,
            });
        }
        if let PanelInfo::Stack { sizes, .. } = &mut panel.info
            && ignore_sizes
        {
            sizes.clear();
        }
        for child in &mut panel.children {
            normalize(child, ignore_sizes);
        }
    }
    let mut state = state.clone();
    state.next_id = 0;
    state.paused = false;
    for window in &mut state.windows {
        window.bounds = [0., 0., 1., 1.];
        normalize(&mut window.dock.center, ignore_sizes);
        for dock in [
            &mut window.dock.left_dock,
            &mut window.dock.right_dock,
            &mut window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            let mut panel = dock.panel().clone();
            normalize(&mut panel, ignore_sizes);
            *dock = DockState::new(
                panel,
                dock.placement(),
                if ignore_sizes { px(0.) } else { dock.size() },
                dock.open(),
            );
        }
    }
    serde_json::to_value(state).expect("workspace is serializable")
}

fn geometry_changed(before: &WorkspaceFile, after: &WorkspaceFile) -> bool {
    before.windows.iter().any(|old| {
        after
            .windows
            .iter()
            .find(|new| new.id == old.id)
            .is_some_and(|new| old.bounds != new.bounds)
    })
}

fn push_history(state: WorkspaceFile, cx: &mut App) {
    let history = &mut cx.global_mut::<Session>().history;
    if history
        .last()
        .is_none_or(|last| layout_key(last, false) != layout_key(&state, false))
    {
        history.push(state);
        if history.len() > 20 {
            history.remove(0);
        }
    }
}

pub(super) fn record(cx: &mut App) {
    if cx.global::<Session>().loading {
        return;
    }
    let current = snapshot(cx);
    let previous = cx.global_mut::<Session>().last.replace(current.clone());
    if let Some(previous) = previous
        && serde_json::to_value(&previous).ok() != serde_json::to_value(&current).ok()
    {
        mark_dirty(cx);
        // Resizing a native window also rescales its dock slots. Treat that
        // geometry as one native operation, not a new layout undo entry.
        let ignore_sizes = geometry_changed(&previous, &current);
        if layout_key(&previous, ignore_sizes) != layout_key(&current, ignore_sizes) {
            push_history(previous, cx);
        }
    }
}

/// Capture entities on the UI thread, then serialize and commit in the background.
/// One in-flight save bounds the queue; newer edits stay dirty until the next tick.
pub(super) fn save_now(cx: &mut App) {
    let session = cx.global::<Session>();
    if session.loading || session.save_task.is_some() {
        return;
    }
    let Some(writer) = session.writer.clone() else {
        return;
    };
    let state = snapshot_board_set(cx);
    let session = cx.global_mut::<Session>();
    session.dirty = true;
    let revision = session.save_revision.begin();
    let background_writer = writer.clone();
    let write = cx
        .background_executor()
        .spawn(async move { layout_persistence::save(&background_writer, revision, &state) });
    let task = cx.spawn(async move |cx| {
        let result = write.await;
        cx.update(|cx| {
            if !cx.has_global::<Session>() {
                return;
            }
            let session = cx.global_mut::<Session>();
            if !session
                .writer
                .as_ref()
                .is_some_and(|current| std::sync::Arc::ptr_eq(current, &writer))
            {
                return;
            }
            let Some(current) = session.save_revision.finish(revision) else {
                return;
            };
            session.save_task = None;
            match result {
                Ok(()) if current => {
                    session.dirty = false;
                    if session.status.starts_with("布局保存失败：") {
                        session.status = "布局已保存".into();
                    }
                }
                Ok(()) => {}
                Err(error) => session.status = format!("布局保存失败：{error}"),
            }
            cx.refresh_windows();
        });
    });
    cx.global_mut::<Session>().save_task = Some(task);
}

pub(super) fn saved_panel_ids(state: &WorkspaceFile) -> std::collections::HashSet<u64> {
    fn collect(panel: &PanelState, ids: &mut std::collections::HashSet<u64>) {
        if panel.panel_name == "tydaro.panel"
            && let PanelInfo::Panel(value) = &panel.info
            && let Ok(saved) = serde_json::from_value::<SavedPanel>(value.clone())
        {
            ids.insert(saved.model.id);
        }
        for child in &panel.children {
            collect(child, ids);
        }
    }
    let mut ids = std::collections::HashSet::new();
    for window in &state.windows {
        collect(&window.dock.center, &mut ids);
        for dock in [
            &window.dock.left_dock,
            &window.dock.right_dock,
            &window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            collect(dock.panel(), &mut ids);
        }
    }
    ids
}

fn apply_workspace(state: &WorkspaceFile, cx: &mut App) -> anyhow::Result<()> {
    // Allocate native destinations before replacing any live layout. A newly
    // created window starts empty; its saved panels are constructed exactly once.
    for saved in &state.windows {
        if workspace(saved.id, cx).is_none() {
            open_workspace(
                saved.id,
                Some(SavedWindow {
                    id: saved.id,
                    bounds: saved.bounds,
                    locked: saved.locked,
                    dock: DockAreaState {
                        version: Some(1),
                        center: empty_tabs(),
                        ..Default::default()
                    },
                }),
                cx,
            )?;
        }
    }
    for saved in &state.windows {
        let view = workspace(saved.id, cx)
            .ok_or_else(|| anyhow::anyhow!("目标窗口 {} 已关闭", saved.id))?;
        let handle = view.read(cx).handle;
        let dock = view.read(cx).dock.clone();
        cx.global_mut::<Session>().building_owner = saved.id;
        let bounds = handle.update(cx, |_, window, cx| -> anyhow::Result<[f32; 4]> {
            dock.update(cx, |dock, cx| {
                dock.load(saved.dock.clone(), window, cx)?;
                dock.set_locked(saved.locked, window, cx);
                Ok::<_, anyhow::Error>(())
            })?;
            // GPUI exposes resize but no programmatic window-position setter.
            // Reused windows keep their current screen position; fresh windows
            // and the next app launch use the saved, visibility-checked position.
            let min_width = if saved.id == 0 { 1024. } else { 240. };
            let min_height = if saved.id == 0 { 680. } else { 240. };
            let requested = size(
                px(saved.bounds[2].max(min_width)),
                px(saved.bounds[3].max(min_height)),
            );
            if window.bounds().size != requested {
                window.resize(requested);
            }
            Ok(window_bounds(window))
        })??;
        view.update(cx, |view, _| view.bounds = bounds);
    }
    // Retire source windows only after every destination was loaded. The
    // conditional on_removed callback cannot delete a newly registered entity
    // with the same durable panel ID.
    let obsolete: Vec<_> = cx
        .global::<Session>()
        .windows
        .keys()
        .copied()
        .filter(|id| !state.windows.iter().any(|saved| saved.id == *id))
        .collect();
    for id in obsolete {
        close_window(id, cx);
    }
    let ids = saved_panel_ids(state);
    let session = cx.global_mut::<Session>();
    session
        .panels
        .retain(|id, view| ids.contains(id) && view.upgrade().is_some());
    session.next_id = session.next_id.max(state.next_id);
    session.paused = state.paused;
    session.drag = DragCoordinator::default();
    Ok(())
}

pub(crate) fn restore(state: WorkspaceFile, cx: &mut App) -> anyhow::Result<()> {
    crate::workspace::snapshot::validate(&state)?;
    anyhow::ensure!(
        state.windows.iter().any(|window| window.id == 0),
        "布局缺少主窗口"
    );
    let before = snapshot(cx);
    let previous_dirty = cx.global::<Session>().dirty;
    cx.global_mut::<Session>().loading = true;
    let result = apply_workspace(&state, cx);
    if let Err(error) = result {
        let rollback = apply_workspace(&before, cx);
        cx.global_mut::<Session>().loading = false;
        cx.global_mut::<Session>().last = Some(snapshot(cx));
        cx.global_mut::<Session>().dirty = previous_dirty;
        refresh_market(cx);
        return match rollback {
            Ok(()) => Err(anyhow::anyhow!("布局未应用，已恢复原工作区：{error}")),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "布局恢复失败：{error}；回滚也失败：{rollback_error}"
            )),
        };
    }
    cx.global_mut::<Session>().loading = false;
    refresh_market(cx);
    cx.global_mut::<Session>().last = Some(snapshot(cx));
    changed(cx);
    Ok(())
}

// Keep live business/view state when undoing arrangement changes. A panel that
// was actually closed has no live counterpart and recovers its saved content.
fn preserve_live_state(target: &mut WorkspaceFile, current: &WorkspaceFile) {
    fn collect(panel: &PanelState, panels: &mut HashMap<u64, SavedPanel>) {
        if panel.panel_name == "tydaro.panel"
            && let PanelInfo::Panel(value) = &panel.info
            && let Ok(saved) = serde_json::from_value::<SavedPanel>(value.clone())
        {
            panels.insert(saved.model.id, saved);
        }
        for child in &panel.children {
            collect(child, panels);
        }
    }
    fn apply(panel: &mut PanelState, panels: &HashMap<u64, SavedPanel>) {
        if panel.panel_name == "tydaro.panel"
            && let PanelInfo::Panel(value) = &mut panel.info
            && let Ok(mut saved) = serde_json::from_value::<SavedPanel>(value.clone())
            && let Some(live) = panels.get(&saved.model.id)
        {
            saved.model.symbol = live.model.symbol;
            saved.model.period = live.model.period;
            saved.model.custom_name = live.model.custom_name.clone();
            saved.content = live.content.clone();
            *value = serde_json::to_value(saved).expect("saved panel is serializable");
        }
        for child in &mut panel.children {
            apply(child, panels);
        }
    }
    let mut panels = HashMap::new();
    for window in &current.windows {
        collect(&window.dock.center, &mut panels);
        for dock in [
            &window.dock.left_dock,
            &window.dock.right_dock,
            &window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            collect(dock.panel(), &mut panels);
        }
    }
    for window in &mut target.windows {
        if let Some(live) = current.windows.iter().find(|live| live.id == window.id) {
            window.bounds = live.bounds;
        }
        apply(&mut window.dock.center, &panels);
        for dock in [
            &mut window.dock.left_dock,
            &mut window.dock.right_dock,
            &mut window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            let mut panel = dock.panel().clone();
            apply(&mut panel, &panels);
            *dock = DockState::new(panel, dock.placement(), dock.size(), dock.open());
        }
    }
    target.paused = current.paused;
    target.next_id = target.next_id.max(current.next_id);
}

pub(crate) fn undo(cx: &mut App) {
    // Capture a just-completed drag even if the autosave timer has not fired.
    record(cx);
    let Some(mut state) = cx.global::<Session>().history.last().cloned() else {
        status("暂无可撤销的布局修改", cx);
        return;
    };
    preserve_live_state(&mut state, &snapshot(cx));
    match restore(state, cx) {
        Ok(()) => {
            cx.global_mut::<Session>().history.pop();
            status("已撤销布局修改", cx);
        }
        Err(error) => report(Err(error), cx),
    }
}

pub(super) fn reset(dual: bool, cx: &mut App) {
    record(cx);
    let Some(view) = workspace(0, cx) else {
        status("主窗口不可用，无法切换布局", cx);
        return;
    };
    let before = snapshot(cx);
    let handle = view.read(cx).handle;
    let dock = view.read(cx).dock.clone();
    cx.global_mut::<Session>().loading = true;
    let result = handle.update(cx, |_, window, cx| {
        default_layout(&dock, 0, dual, window, cx);
        dock.update(cx, |dock, cx| dock.set_locked(false, window, cx));
    });
    if let Err(error) = result {
        cx.global_mut::<Session>().loading = false;
        report(Err(error), cx);
        return;
    }
    let secondary: Vec<_> = cx
        .global::<Session>()
        .windows
        .keys()
        .copied()
        .filter(|id| *id != 0)
        .collect();
    for id in secondary {
        close_window(id, cx);
    }
    let current = snapshot(cx);
    let ids = saved_panel_ids(&current);
    cx.global_mut::<Session>()
        .panels
        .retain(|id, view| ids.contains(id) && view.upgrade().is_some());
    cx.global_mut::<Session>().drag = DragCoordinator::default();
    cx.global_mut::<Session>().loading = false;
    push_history(before, cx);
    refresh_market(cx);
    cx.global_mut::<Session>().last = Some(snapshot(cx));
    changed(cx);
}

pub(super) fn template(save: bool, cx: &mut App) {
    let Some(path) = cx
        .global::<Session>()
        .path
        .as_ref()
        .map(|path| path.with_file_name("template.json"))
    else {
        return;
    };
    let result = if save {
        storage::write(&path, &snapshot(cx))
    } else {
        storage::read(&path).and_then(|mut state| {
            // A template may have originated from another board. Assign fresh
            // identities so its panels cannot alias inactive boards or history.
            let next_id = snapshot_board_set(cx).next_entity_id();
            remap_workspace_ids(&mut state, next_id)?;
            state.paused = cx.global::<Session>().paused;
            keep_main_geometry(&mut state, cx);
            record(cx);
            let before = snapshot(cx);
            restore(state, cx)?;
            push_history(before, cx);
            Ok(())
        })
    };
    match result {
        Ok(()) => status(
            if save {
                "已保存自定义布局模板"
            } else {
                "已加载自定义布局模板"
            },
            cx,
        ),
        Err(error) => status(format!("模板操作失败：{error}"), cx),
    }
}

/// Closing is a durability boundary. Cancel the callback, wait for any active
/// write, then persist a fresh snapshot under the same lock before quitting.
pub(crate) fn flush(cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(
        !cx.global::<Session>().loading,
        "布局正在恢复，暂时无法保存"
    );
    let Some(writer) = cx.global::<Session>().writer.clone() else {
        return Ok(());
    };
    let state = snapshot_board_set(cx);
    let session = cx.global_mut::<Session>();
    session.save_task = None;
    session.dirty = true;
    let revision = session.save_revision.begin();
    let result = layout_persistence::save(&writer, revision, &state);
    let session = cx.global_mut::<Session>();
    let _ = session.save_revision.finish(revision);
    match &result {
        Ok(()) => {
            session.dirty = false;
            if session.status.starts_with("布局保存失败：") {
                session.status = "布局已保存".into();
            }
        }
        Err(error) => session.status = format!("布局保存失败：{error}"),
    }
    result
}

#[cfg(test)]
mod tests;
