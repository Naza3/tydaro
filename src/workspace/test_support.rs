//! Assertions and fixture operations for native workspace regression scenarios.
use super::{Session, changed, close_window, move_between, panel, restore, snapshot, undo};
use super::{
    persistence::{layout_key, reset, saved_panel_ids},
    transfer,
};
use gpui_kit::*;

pub(crate) fn live_panel_ids(cx: &App) -> Vec<u64> {
    // Include stale weak entries so regression checks also catch registry leaks.
    let mut ids: Vec<_> = cx.global::<Session>().panels.keys().copied().collect();
    ids.sort_unstable();
    ids
}

pub(crate) fn verify_transfer_geometry(cx: &mut App) -> anyhow::Result<()> {
    let windows: Vec<_> = cx
        .global::<Session>()
        .windows
        .values()
        .filter_map(WeakEntity::upgrade)
        .collect();
    for workspace in windows {
        let view = workspace.read(cx);
        let id = view.id;
        let handle = view.handle;
        let dock = view.dock.clone();
        handle.update(cx, |_, window, cx| {
            let expected = transfer::screen_rect(window, dock.read(cx).bounds());
            cx.global::<Session>()
                .drag
                .verify_window_geometry(id, expected)
        })??;
    }
    Ok(())
}
pub(crate) fn return_window(id: u64, cx: &mut App) -> anyhow::Result<()> {
    if id == 0 {
        return Ok(());
    }
    let ids: Vec<_> = cx
        .global::<Session>()
        .panels
        .values()
        .filter_map(WeakEntity::upgrade)
        .filter(|p| p.read(cx).owner == id)
        .map(|p| p.read(cx).model.id)
        .collect();
    for panel in ids {
        move_between(panel, 0, None, cx)?;
    }
    close_window(id, cx);
    changed(cx);
    Ok(())
}
pub(crate) fn verify_workspace_persistence(cx: &mut App) -> anyhow::Result<()> {
    let before = snapshot(cx);
    crate::workspace::snapshot::validate(&before)?;
    let ids = saved_panel_ids(&before);
    restore(before.clone(), cx)?;
    let restored = snapshot(cx);
    anyhow::ensure!(
        before.windows.len() == restored.windows.len(),
        "Restoring changed window count"
    );
    anyhow::ensure!(
        saved_panel_ids(&restored) == ids,
        "Restoring lost panel IDs"
    );
    anyhow::ensure!(
        layout_key(&before, true) == layout_key(&restored, true),
        "Restoring changed the dock tree"
    );
    reset(true, cx);
    undo(cx);
    let undone = snapshot(cx);
    anyhow::ensure!(saved_panel_ids(&undone) == ids, "Undo lost panel IDs");
    anyhow::ensure!(
        layout_key(&before, true) == layout_key(&undone, true),
        "Undo did not restore the multiwindow layout"
    );
    for id in ids {
        anyhow::ensure!(
            panel(id, cx).is_some(),
            "Panel registry lost restored ID {id}"
        );
    }
    Ok(())
}
