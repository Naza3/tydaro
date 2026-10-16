//! Panel creation, duplication and transactional cross-window movement.
use super::{
    Session, StockPanel, allocate, changed, close_if_empty, close_window, floating_placement,
    is_locked, open_workspace_on_display, panel, refresh_market, workspace,
};
use super::{
    snapshot::SavedPanel,
    transfer::{DockSide, ScreenPoint, TransferIntent},
};
use crate::model::{PanelKind, PanelModel};
use gpui_kit::{
    component::{Placement, dock::*},
    *,
};

fn focus_panel(id: u64, cx: &mut App) {
    if let Some(p) = panel(id, cx) {
        let view = p.read(cx);
        let focus = view.focus.clone();
        if let Some(w) = workspace(view.owner, cx) {
            let handle = w.read(cx).handle;
            let _ = handle.update(cx, |_, window, cx| {
                window.activate_window();
                window.focus(&focus, cx);
            });
        }
    }
}

/// All component creation paths use a native, focused destination so the new
/// component cannot silently become an inactive tab in an unrelated group.
fn create_floating(owner: u64, saved: SavedPanel, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(!is_locked(owner, cx), "布局已锁定，请先解锁");
    let (display, bounds) = floating_placement(owner, saved.model.kind, None, cx)?;
    let id = saved.model.id;
    let destination = allocate(cx);
    let w = open_workspace_on_display(destination, None, display, Some(bounds), cx)?;
    let handle = w.read(cx).handle;
    let dock = w.read(cx).dock.clone();
    if let Err(error) = handle
        .update(cx, |_, window, cx| {
            let p = StockPanel::create(destination, saved, window, cx)?;
            dock.update(cx, |d, cx| {
                d.add_panel_view(panel_handle(p), DockPlacement::Center, None, window, cx)
            });
            Ok(())
        })
        .and_then(|result| result)
    {
        close_window(destination, cx);
        return Err(error);
    }
    refresh_market(cx);
    focus_panel(id, cx);
    changed(cx);
    Ok(())
}

pub(crate) fn add_panel(owner: u64, kind: PanelKind, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(
        cx.global::<Session>()
            .panels
            .values()
            .filter_map(WeakEntity::upgrade)
            .count()
            < 128,
        "最多支持 128 个组件"
    );
    anyhow::ensure!(!is_locked(owner, cx), "布局已锁定，请先解锁");
    anyhow::ensure!(workspace(owner, cx).is_some(), "源窗口已关闭");
    anyhow::ensure!(
        cx.global::<Session>().windows.len() < 16,
        "最多支持 16 个工作区窗口"
    );
    let id = allocate(cx);
    let mut model = PanelModel::new(id, kind);
    // Prefer the invoking workspace's linked market context. This remains
    // deterministic with multiple independent groups and preserves duplication
    // separately through its complete SavedPanel snapshot.
    let context = cx
        .global::<Session>()
        .panels
        .values()
        .filter_map(WeakEntity::upgrade)
        .filter_map(|p| {
            let p = p.read(cx);
            (p.owner == owner && p.model.link_group != 0).then(|| p.model.clone())
        })
        .min_by_key(|p| p.id);
    if let Some(context) = context {
        model.symbol = context.symbol;
        model.link_group = context.link_group;
    }
    create_floating(
        owner,
        SavedPanel {
            model,
            content: serde_json::Value::Null,
        },
        cx,
    )
}

pub(crate) fn duplicate(id: u64, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(
        cx.global::<Session>()
            .panels
            .values()
            .filter_map(WeakEntity::upgrade)
            .count()
            < 128,
        "最多支持 128 个组件"
    );
    let p = panel(id, cx).ok_or_else(|| anyhow::anyhow!("组件已关闭"))?;
    let owner = p.read(cx).owner;
    anyhow::ensure!(!is_locked(owner, cx), "布局已锁定，请先解锁");
    anyhow::ensure!(
        cx.global::<Session>().windows.len() < 16,
        "最多支持 16 个工作区窗口"
    );
    let mut saved = p.read(cx).saved(cx);
    saved.model.id = allocate(cx);
    create_floating(owner, saved, cx)
}
fn dock_target(dock: &DockArea, target: Option<(u64, DockSide)>, cx: &App) -> Option<InsertTarget> {
    let (id, side) = target?;
    let p = panel(id, cx)?;
    let tree = dock.layout(DockPlacement::Center)?;
    let node = tree.find_panel_node(PanelId::from(p.entity_id()))?;
    Some(match side {
        DockSide::Center => InsertTarget::Tabs {
            node,
            ix: None,
            activate: true,
        },
        s => InsertTarget::Split {
            node,
            placement: match s {
                DockSide::Left => Placement::Left,
                DockSide::Right => Placement::Right,
                DockSide::Top => Placement::Top,
                _ => Placement::Bottom,
            },
            size: None,
        },
    })
}
pub(crate) fn move_between(
    id: u64,
    destination: u64,
    target: Option<(u64, DockSide)>,
    cx: &mut App,
) -> anyhow::Result<()> {
    let p = panel(id, cx).ok_or_else(|| anyhow::anyhow!("组件已关闭"))?;
    let source = p.read(cx).owner;
    anyhow::ensure!(
        !is_locked(source, cx) && !is_locked(destination, cx),
        "布局已锁定，请先解锁"
    );
    if let Some((target_id, _)) = target {
        if target_id == id {
            anyhow::ensure!(source == destination, "目标组件不属于目标窗口");
            return Ok(());
        }
        let target_panel = panel(target_id, cx).ok_or_else(|| anyhow::anyhow!("目标组件已关闭"))?;
        anyhow::ensure!(
            target_panel.read(cx).owner == destination,
            "目标组件已移动，请重新选择位置"
        );
    }
    let dest = workspace(destination, cx).ok_or_else(|| anyhow::anyhow!("目标窗口已关闭"))?;
    let dest_handle = dest.read(cx).handle;
    let dest_dock = dest.read(cx).dock.clone();
    let insertion = dock_target(dest_dock.read(cx), target, cx);
    anyhow::ensure!(
        target.is_none() || insertion.is_some(),
        "目标组件不在可停靠区域内"
    );
    if source == destination {
        if let Some(insertion) = insertion {
            dest_handle.update(cx, |_, window, cx| {
                dest_dock.update(cx, |d, cx| {
                    d.move_panel(PanelId::from(p.entity_id()), insertion, window, cx)
                })
            })?;
        }
        focus_panel(id, cx);
        changed(cx);
        return Ok(());
    }
    let saved = p.read(cx).saved(cx);
    let source_workspace = workspace(source, cx).ok_or_else(|| anyhow::anyhow!("源窗口已关闭"))?;
    let source_handle = source_workspace.read(cx).handle;
    let source_dock = source_workspace.read(cx).dock.clone();
    // Construct the destination before removing the source. The new entity has
    // window-local input/focus handles and the same durable panel identifier.
    let moved = dest_handle.update(cx, |_, window, cx| {
        let moved = StockPanel::create(destination, saved, window, cx)?;
        dest_dock.update(cx, |d, cx| {
            d.add_panel_view(
                panel_handle(moved.clone()),
                DockPlacement::Center,
                None,
                window,
                cx,
            );
            if let Some(insertion) = insertion {
                d.move_panel(PanelId::from(moved.entity_id()), insertion, window, cx);
            }
        });
        Ok::<_, anyhow::Error>(moved)
    })??;
    if let Err(err) = source_handle.update(cx, |_, window, cx| {
        source_dock.update(cx, |d, cx| d.remove_panel(p.clone(), window, cx))
    }) {
        let _ = dest_handle.update(cx, |_, window, cx| {
            dest_dock.update(cx, |d, cx| d.remove_panel(moved, window, cx))
        });
        cx.global_mut::<Session>().panels.insert(id, p.downgrade());
        return Err(err);
    }
    refresh_market(cx);
    focus_panel(id, cx);
    changed(cx);
    close_if_empty(source, cx);
    Ok(())
}
pub(crate) fn detach(id: u64, position: Option<ScreenPoint>, cx: &mut App) -> anyhow::Result<()> {
    let source = panel(id, cx).ok_or_else(|| anyhow::anyhow!("组件已关闭"))?;
    let owner = source.read(cx).owner;
    anyhow::ensure!(!is_locked(owner, cx), "布局已锁定，请先解锁");
    let kind = source.read(cx).model.kind;
    let (display_id, bounds) = floating_placement(owner, kind, position, cx)?;
    let win = allocate(cx);
    // Start with an empty live dock. Loading DockAreaState::default() would
    // deserialize an invalid unnamed placeholder as an extra tab.
    open_workspace_on_display(win, None, display_id, Some(bounds), cx)?;
    if let Err(error) = move_between(id, win, None, cx) {
        close_window(win, cx);
        return Err(error);
    }
    Ok(())
}
pub(super) fn perform_transfer(intent: TransferIntent, cx: &mut App) -> anyhow::Result<()> {
    match intent {
        TransferIntent::Dock {
            source_window,
            panel,
            target,
        } => {
            let source =
                workspace(source_window, cx).ok_or_else(|| anyhow::anyhow!("源窗口已关闭"))?;
            let destination =
                workspace(target.window, cx).ok_or_else(|| anyhow::anyhow!("目标窗口已关闭"))?;
            anyhow::ensure!(
                !source.read(cx).dock.read(cx).is_locked()
                    && !destination.read(cx).dock.read(cx).is_locked(),
                "布局已锁定，请先解锁"
            );
            move_between(
                panel,
                target.window,
                target.panel.map(|p| (p, target.side)),
                cx,
            )
        }
        TransferIntent::Detach {
            source_window,
            panel,
            position,
        } => {
            let source =
                workspace(source_window, cx).ok_or_else(|| anyhow::anyhow!("源窗口已关闭"))?;
            anyhow::ensure!(
                !source.read(cx).dock.read(cx).is_locked(),
                "布局已锁定，请先解锁"
            );
            detach(panel, Some(position), cx)
        }
    }
}
