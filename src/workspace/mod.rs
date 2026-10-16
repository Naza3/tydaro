//! Native workspaces, persistent dock trees and cross-window panel ownership.
//!
//! This facade owns the one live Session. Child modules implement commands and
//! adapters; no application module can reach the registries or mutate Session.
pub(crate) mod board_bar;
mod board_ops;
pub(crate) mod boards;
mod layout;
mod operations;
mod panel;
mod persistence;
mod runtime;
pub(crate) mod snapshot;
#[cfg(feature = "test-support")]
mod test_support;
pub(crate) mod transfer;
mod view;
mod windows;

pub(crate) use board_ops::{
    active_board_id, board_summaries, create_board, delete_board, duplicate_board, move_board,
    rename_board, restore_board_set, snapshot_board_set, switch_board,
};
pub(crate) use layout::BoardPreset;
pub(crate) use operations::{add_panel, detach, duplicate, move_between};
pub(crate) use persistence::{flush, restore, snapshot, undo};
pub(crate) use runtime::{
    advance_market, init, maintain_layout, market_tick, open_initial, open_market_symbol,
};
#[cfg(feature = "test-support")]
pub(crate) use test_support::{
    live_panel_ids, return_window, verify_transfer_geometry, verify_workspace_persistence,
};
pub(crate) use view::{market_toolbar, register_main_dock};
#[cfg(feature = "test-support")]
pub(crate) use windows::dismiss_window;
pub(crate) use windows::{Workspace, open_workspace};

use crate::adapters::layout::writer::{SaveRevision, SharedWriter};
use board_ops::{active_board_name, keep_main_geometry};
use boards::BoardSetFile;
use gpui_kit::{component::dock::DockArea, *};
use layout::{default_layout, empty_tabs, preset_workspace, remap_workspace_ids};
use operations::perform_transfer;
use panel::{StockPanel, is_locked};
use persistence::{record, reset, save_now, template};
use runtime::{refresh_market, select_symbol};
use snapshot::WorkspaceFile;
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
};
use transfer::DragCoordinator;
use windows::{
    close_if_empty, close_window, floating_placement, open_workspace_on_display, window_bounds,
};

/// The application supplies its shell and close policy. Workspaces do not know
/// which pages, services or design tools the host application contains.
#[derive(Clone, Copy)]
pub(crate) struct WorkspaceHost {
    pub build_main: fn(Entity<DockArea>, &mut Window, &mut App) -> AnyView,
    pub close_main: fn(&mut Window, &mut App) -> bool,
    pub market_visible: fn(&App) -> bool,
}

struct Session {
    host: WorkspaceHost,
    windows: BTreeMap<u64, WeakEntity<Workspace>>,
    panels: HashMap<u64, WeakEntity<StockPanel>>,
    next_id: u64,
    tick: u64,
    paused: bool,
    dirty: bool,
    loading: bool,
    building_owner: u64,
    path: Option<PathBuf>,
    writer: Option<SharedWriter>,
    save_task: Option<Task<()>>,
    save_revision: SaveRevision,
    maintenance_ticks: u64,
    status: String,
    drag: DragCoordinator,
    history: Vec<WorkspaceFile>,
    last: Option<WorkspaceFile>,
    boards: Option<BoardSetFile>,
    board_histories: HashMap<u64, Vec<WorkspaceFile>>,
}
impl Global for Session {}
fn allocate(cx: &mut App) -> u64 {
    let s = cx.global_mut::<Session>();
    let id = s.next_id;
    s.next_id += 1;
    id
}
pub(crate) fn workspace(id: u64, cx: &App) -> Option<Entity<Workspace>> {
    cx.global::<Session>().windows.get(&id)?.upgrade()
}

fn panel(id: u64, cx: &App) -> Option<Entity<StockPanel>> {
    cx.global::<Session>().panels.get(&id)?.upgrade()
}
fn status(message: impl Into<String>, cx: &mut App) {
    cx.global_mut::<Session>().status = message.into();
    cx.refresh_windows();
}
pub(crate) fn status_message(cx: &App) -> String {
    cx.global::<Session>().status.clone()
}
fn changed(cx: &mut App) {
    if !cx.global::<Session>().loading {
        mark_dirty(cx);
    }
    cx.refresh_windows();
}
fn mark_dirty(cx: &mut App) {
    let session = cx.global_mut::<Session>();
    session.dirty = true;
    session.save_revision.changed();
}
fn report(result: anyhow::Result<()>, cx: &mut App) {
    if let Err(error) = result {
        status(format!("操作失败：{error}"), cx);
    }
}
