//! Initialization, market linkage and periodic maintenance of the live session.
use super::transfer::DragCoordinator;
use super::{
    Session, StockPanel, WorkspaceHost, add_panel, changed, open_workspace, panel, record, report,
    restore_board_set, save_now, snapshot, status, workspace,
};
use crate::adapters::layout as storage;
use crate::adapters::layout::writer::{SaveRevision, WriteSession};
use crate::model::PanelKind;
use gpui_kit::{
    component::dock::{PanelInfo, panel_handle, register_panel},
    *,
};
use std::collections::{BTreeMap, HashMap};

pub(super) fn refresh_market(cx: &mut App) {
    let tick = cx.global::<Session>().tick;
    let panels: Vec<_> = cx
        .global::<Session>()
        .panels
        .values()
        .filter_map(WeakEntity::upgrade)
        .collect();
    for p in panels {
        p.update(cx, |p, cx| {
            p.content.update(cx, |c, cx| {
                c.set_market(p.model.symbol, p.model.period, tick, cx)
            });
            cx.notify();
        });
    }
}
pub(super) fn select_symbol(id: u64, symbol: crate::market::Symbol, cx: &mut App) {
    let Some(source) = panel(id, cx) else { return };
    let group = source.read(cx).model.link_group;
    let peers: Vec<_> = cx
        .global::<Session>()
        .panels
        .values()
        .filter_map(WeakEntity::upgrade)
        .filter(|p| {
            p.read(cx).model.id == id || (group != 0 && p.read(cx).model.link_group == group)
        })
        .collect();
    for p in peers {
        p.update(cx, |p, _| p.model.symbol = symbol);
    }
    refresh_market(cx);
    changed(cx);
}

pub(crate) fn market_tick(cx: &App) -> u64 {
    cx.global::<Session>().tick
}
pub(crate) fn open_market_symbol(symbol: crate::market::Symbol, cx: &mut App) {
    let id = cx
        .global::<Session>()
        .panels
        .values()
        .filter_map(WeakEntity::upgrade)
        .filter_map(|p| {
            let p = p.read(cx);
            (p.owner == 0 && p.model.kind == PanelKind::Chart).then_some(p.model.id)
        })
        .min();
    if let Some(id) = id {
        select_symbol(id, symbol, cx);
    } else {
        match add_panel(0, PanelKind::Chart, cx) {
            Ok(()) => {
                let newest = cx
                    .global::<Session>()
                    .panels
                    .values()
                    .filter_map(WeakEntity::upgrade)
                    .filter_map(|p| {
                        let p = p.read(cx);
                        (p.model.kind == PanelKind::Chart).then_some(p.model.id)
                    })
                    .max();
                if let Some(id) = newest {
                    select_symbol(id, symbol, cx);
                }
            }
            Err(error) => status(format!("打开行情失败：{error}"), cx),
        }
    }
}

pub(crate) fn init(cx: &mut App, persistent: bool, host: WorkspaceHost) {
    let path = persistent.then(|| storage::data_dir().join("workspace.json"));
    let writer = path.clone().map(WriteSession::shared);
    cx.set_global(Session {
        host,
        windows: BTreeMap::new(),
        panels: HashMap::new(),
        next_id: 1,
        tick: 0,
        paused: false,
        dirty: false,
        loading: false,
        building_owner: 0,
        path,
        writer,
        save_task: None,
        save_revision: SaveRevision::default(),
        maintenance_ticks: 0,
        status: String::new(),
        drag: DragCoordinator::default(),
        history: vec![],
        last: None,
        boards: None,
        board_histories: HashMap::new(),
    });
    register_panel(cx, "tydaro.panel", |build, window, cx| {
        let saved = match build.info() {
            PanelInfo::Panel(value) => {
                serde_json::from_value(value.clone()).expect("validated panel payload")
            }
            _ => unreachable!("validated panel info"),
        };
        let owner = cx.global::<Session>().building_owner;
        panel_handle(
            StockPanel::create(owner, saved, window, cx)
                .expect("dock payload was validated before loading"),
        )
    });
}
/// Restore the user's workspaces after the application has installed services.
pub(crate) fn open_initial(cx: &mut App) -> anyhow::Result<()> {
    let path = cx.global::<Session>().path.clone();
    let saved =
        path.filter(|path| path.exists())
            .and_then(|path| match storage::read_board_set(&path) {
                Ok(state) => Some(state),
                Err(error) => {
                    status(format!("布局读取失败，已使用默认布局：{error}"), cx);
                    None
                }
            });
    if let Some(state) = saved {
        let result = restore_board_set(state, cx);
        report(result, cx);
    }
    if workspace(0, cx).is_none() {
        open_workspace(0, None, cx)?;
    }
    cx.global_mut::<Session>().last = Some(snapshot(cx));
    Ok(())
}

/// A display update is independent from layout persistence scheduling.
pub(crate) fn advance_market(cx: &mut App) {
    let session = cx.global_mut::<Session>();
    if session.paused {
        return;
    }
    session.tick += 1;
    refresh_market(cx);
    cx.refresh_windows();
}

pub(crate) fn maintain_layout(cx: &mut App) {
    let session = cx.global_mut::<Session>();
    session.maintenance_ticks = session.maintenance_ticks.wrapping_add(1);
    let should_record = session.dirty || session.maintenance_ticks.is_multiple_of(5);
    if should_record {
        record(cx);
        if cx.global::<Session>().dirty {
            save_now(cx);
        }
    }
}
