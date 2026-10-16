//! Commands over the board set; only the active board owns native entities.
use super::{
    BoardPreset, Session, changed, preset_workspace, record, remap_workspace_ids, restore,
    save_now, snapshot, status, workspace,
};
use super::{
    boards::{self, BoardSetFile, SavedBoard},
    snapshot::WorkspaceFile,
};
use gpui_kit::*;

pub(crate) fn active_board_id(cx: &App) -> u64 {
    cx.global::<Session>()
        .boards
        .as_ref()
        .map_or(1, |state| state.active_board)
}

pub(crate) fn board_summaries(cx: &App) -> Vec<(u64, String)> {
    cx.global::<Session>().boards.as_ref().map_or_else(
        || vec![(1, "默认看板".into())],
        |state| {
            state
                .boards
                .iter()
                .map(|b| (b.id, b.name.clone()))
                .collect()
        },
    )
}

pub(super) fn active_board_name(cx: &App) -> String {
    let id = active_board_id(cx);
    board_summaries(cx)
        .into_iter()
        .find(|(board, _)| *board == id)
        .map_or_else(|| "默认看板".into(), |(_, name)| name)
}

/// Inactive boards remain durable values; only the selected board owns live
/// windows. Patch the selected value at every snapshot so inputs and viewports
/// are saved even when they did not emit a layout event.
pub(crate) fn snapshot_board_set(cx: &App) -> BoardSetFile {
    let live = snapshot(cx);
    let Some(mut state) = cx.global::<Session>().boards.clone() else {
        return BoardSetFile::from_legacy(live);
    };
    if let Some(board) = state.boards.iter_mut().find(|b| b.id == state.active_board) {
        board.workspace = live;
    }
    state
}

pub(super) fn keep_main_geometry(state: &mut WorkspaceFile, cx: &App) {
    if let Some(main) = workspace(0, cx)
        && let Some(saved) = state.windows.iter_mut().find(|w| w.id == 0)
    {
        saved.bounds = main.read(cx).bounds;
    }
}

/// Validate and load the complete destination before committing its identity or
/// history. `restore` rolls back all window/panel ownership on a load failure.
fn activate_board_set(mut state: BoardSetFile, cx: &mut App) -> anyhow::Result<()> {
    boards::validate(&state)?;
    let target_id = state.active_board;
    let source_id = active_board_id(cx);
    let target = state
        .boards
        .iter_mut()
        .find(|b| b.id == target_id)
        .expect("validated active board");
    target.workspace.paused = cx.global::<Session>().paused;
    keep_main_geometry(&mut target.workspace, cx);
    let target_workspace = target.workspace.clone();
    record(cx);
    // Allocation never goes backwards, including while restoring an older
    // board or rolling back a failed transition.
    cx.global_mut::<Session>().next_id = cx.global::<Session>().next_id.max(state.next_entity_id());
    restore(target_workspace, cx)?;
    let live = snapshot(cx);
    let session = cx.global_mut::<Session>();
    let old_history = std::mem::take(&mut session.history);
    session.board_histories.insert(source_id, old_history);
    session.history = session
        .board_histories
        .remove(&target_id)
        .unwrap_or_default();
    session.last = Some(live);
    session.boards = Some(state);
    changed(cx);
    Ok(())
}

pub(crate) fn switch_board(id: u64, cx: &mut App) -> anyhow::Result<()> {
    let mut state = snapshot_board_set(cx);
    anyhow::ensure!(state.boards.iter().any(|b| b.id == id), "看板不存在");
    if state.active_board == id {
        return Ok(());
    }
    state.active_board = id;
    activate_board_set(state, cx)?;
    status(format!("已切换到 {}", active_board_name(cx)), cx);
    save_now(cx);
    Ok(())
}

fn unique_board_name(
    name: &str,
    except: Option<u64>,
    state: &BoardSetFile,
) -> anyhow::Result<String> {
    let name = boards::validate_board_name(name)?;
    anyhow::ensure!(
        !state
            .boards
            .iter()
            .any(|b| Some(b.id) != except && b.name == name),
        "已存在同名看板"
    );
    Ok(name)
}

fn insert_board(name: &str, workspace: WorkspaceFile, cx: &mut App) -> anyhow::Result<u64> {
    let mut state = snapshot_board_set(cx);
    let name = unique_board_name(name, None, &state)?;
    anyhow::ensure!(
        state.boards.len() < boards::MAX_BOARDS,
        "最多支持 16 个看板"
    );
    anyhow::ensure!(
        state.next_board_id < crate::workspace::snapshot::MAX_STABLE_ID,
        "看板编号已用尽"
    );
    let id = state.next_board_id;
    state.next_board_id += 1;
    state.boards.push(SavedBoard {
        id,
        name,
        workspace,
    });
    state.active_board = id;
    activate_board_set(state, cx)?;
    status(format!("已创建看板 {}", active_board_name(cx)), cx);
    save_now(cx);
    Ok(id)
}

pub(crate) fn create_board(name: &str, preset: BoardPreset, cx: &mut App) -> anyhow::Result<u64> {
    let state = snapshot_board_set(cx);
    unique_board_name(name, None, &state)?;
    anyhow::ensure!(
        state.boards.len() < boards::MAX_BOARDS,
        "最多支持 16 个看板"
    );
    let bounds = workspace(0, cx)
        .map(|w| w.read(cx).bounds)
        .unwrap_or([40., 40., 1536., 930.]);
    let workspace = preset_workspace(
        preset,
        state.next_entity_id().max(cx.global::<Session>().next_id),
        bounds,
        cx.global::<Session>().paused,
    )?;
    insert_board(name, workspace, cx)
}

pub(crate) fn duplicate_board(name: &str, cx: &mut App) -> anyhow::Result<u64> {
    let state = snapshot_board_set(cx);
    unique_board_name(name, None, &state)?;
    anyhow::ensure!(
        state.boards.len() < boards::MAX_BOARDS,
        "最多支持 16 个看板"
    );
    let mut copied = snapshot(cx);
    remap_workspace_ids(
        &mut copied,
        state.next_entity_id().max(cx.global::<Session>().next_id),
    )?;
    insert_board(name, copied, cx)
}

pub(crate) fn rename_board(id: u64, name: &str, cx: &mut App) -> anyhow::Result<()> {
    let mut state = snapshot_board_set(cx);
    let name = unique_board_name(name, Some(id), &state)?;
    let board = state
        .boards
        .iter_mut()
        .find(|b| b.id == id)
        .ok_or_else(|| anyhow::anyhow!("看板不存在"))?;
    board.name = name;
    cx.global_mut::<Session>().boards = Some(state);
    changed(cx);
    save_now(cx);
    Ok(())
}

pub(crate) fn move_board(id: u64, offset: isize, cx: &mut App) -> anyhow::Result<()> {
    let mut state = snapshot_board_set(cx);
    let current = state
        .boards
        .iter()
        .position(|b| b.id == id)
        .ok_or_else(|| anyhow::anyhow!("看板不存在"))?;
    let target = current
        .saturating_add_signed(offset)
        .min(state.boards.len() - 1);
    if target != current {
        let board = state.boards.remove(current);
        state.boards.insert(target, board);
        cx.global_mut::<Session>().boards = Some(state);
        changed(cx);
        save_now(cx);
    }
    Ok(())
}

pub(crate) fn delete_board(id: u64, cx: &mut App) -> anyhow::Result<()> {
    let mut state = snapshot_board_set(cx);
    anyhow::ensure!(state.boards.len() > 1, "至少保留一个看板");
    let index = state
        .boards
        .iter()
        .position(|b| b.id == id)
        .ok_or_else(|| anyhow::anyhow!("看板不存在"))?;
    if state.active_board == id {
        let neighbor = if index + 1 < state.boards.len() {
            index + 1
        } else {
            index - 1
        };
        state.active_board = state.boards[neighbor].id;
        state.boards.remove(index);
        activate_board_set(state, cx)?;
    } else {
        state.boards.remove(index);
        cx.global_mut::<Session>().boards = Some(state);
    }
    cx.global_mut::<Session>().board_histories.remove(&id);
    changed(cx);
    status("已删除看板", cx);
    save_now(cx);
    Ok(())
}

pub(crate) fn restore_board_set(mut state: BoardSetFile, cx: &mut App) -> anyhow::Result<()> {
    boards::validate(&state)?;
    let target = state
        .boards
        .iter_mut()
        .find(|b| b.id == state.active_board)
        .expect("validated active board");
    keep_main_geometry(&mut target.workspace, cx);
    let workspace = target.workspace.clone();
    let next_id = state.next_entity_id().max(cx.global::<Session>().next_id);
    cx.global_mut::<Session>().next_id = next_id;
    restore(workspace, cx)?;
    let live = snapshot(cx);
    let session = cx.global_mut::<Session>();
    session.boards = Some(state);
    session.board_histories.clear();
    session.history.clear();
    session.last = Some(live);
    changed(cx);
    Ok(())
}
