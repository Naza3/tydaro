//! Named, ordered workspaces. A board keeps its complete window and panel tree;
//! only the selected board is materialized into native windows at runtime.

use std::collections::HashSet;

use anyhow::{Result, ensure};
use gpui_kit::component::dock::{PanelInfo, PanelState};
use serde::{Deserialize, Serialize};

use crate::workspace::snapshot::{self, MAX_STABLE_ID, SavedPanel, WorkspaceFile};

pub(crate) const SCHEMA_VERSION: u32 = 2;
pub(crate) const MAX_BOARDS: usize = 16;
const MAX_NAME_CHARACTERS: usize = 24;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SavedBoard {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) workspace: WorkspaceFile,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct BoardSetFile {
    pub(crate) version: u32,
    pub(crate) active_board: u64,
    pub(crate) next_board_id: u64,
    pub(crate) boards: Vec<SavedBoard>,
}

impl BoardSetFile {
    /// The schema 1 payload is retained whole, including floating windows,
    /// unknown panels and saved input state.
    pub(crate) fn from_legacy(workspace: WorkspaceFile) -> Self {
        Self {
            version: SCHEMA_VERSION,
            active_board: 1,
            next_board_id: 2,
            boards: vec![SavedBoard {
                id: 1,
                name: "默认看板".into(),
                workspace,
            }],
        }
    }

    /// Never reuse an identity retained by an inactive board. Older schema 1
    /// files may have a stale `next_id`, so scan the durable trees as well.
    pub(crate) fn next_entity_id(&self) -> u64 {
        let mut next_id = 1;
        for board in &self.boards {
            next_id = next_id.max(board.workspace.next_id);
            for window in &board.workspace.windows {
                next_id = next_id.max(window.id.saturating_add(1));
                scan_panel_ids(&window.dock.center, &mut next_id);
                for dock in [
                    &window.dock.left_dock,
                    &window.dock.right_dock,
                    &window.dock.bottom_dock,
                ]
                .into_iter()
                .flatten()
                {
                    scan_panel_ids(dock.panel(), &mut next_id);
                }
            }
        }
        next_id
    }
}

fn scan_panel_ids(panel: &PanelState, next_id: &mut u64) {
    if panel.panel_name == "tydaro.panel"
        && let PanelInfo::Panel(payload) = &panel.info
        && let Ok(saved) = serde_json::from_value::<SavedPanel>(payload.clone())
    {
        *next_id = (*next_id).max(saved.model.id.saturating_add(1));
    }
    for child in &panel.children {
        scan_panel_ids(child, next_id);
    }
}

fn validate_global_panel_ids(panel: &PanelState, ids: &mut HashSet<u64>) -> Result<()> {
    if panel.panel_name == "tydaro.panel"
        && let PanelInfo::Panel(payload) = &panel.info
    {
        // The workspace validator has already checked the payload shape.
        let saved: SavedPanel = serde_json::from_value(payload.clone())?;
        ensure!(
            ids.insert(saved.model.id),
            "Duplicate panel ID {} across boards",
            saved.model.id
        );
    }
    for child in &panel.children {
        validate_global_panel_ids(child, ids)?;
    }
    Ok(())
}

/// User input is trimmed before storage; Chinese characters count as one
/// character each rather than their UTF-8 byte length.
pub(crate) fn validate_board_name(name: &str) -> Result<String> {
    ensure!(
        !name.chars().any(char::is_control),
        "看板名称不能包含控制字符"
    );
    let name = name.trim();
    ensure!(!name.is_empty(), "请输入看板名称");
    ensure!(
        name.chars().count() <= MAX_NAME_CHARACTERS,
        "看板名称最多 24 个字符"
    );
    Ok(name.to_owned())
}

pub(crate) fn validate(state: &BoardSetFile) -> Result<()> {
    ensure!(
        state.version == SCHEMA_VERSION,
        "Unsupported board schema {} (expected {})",
        state.version,
        SCHEMA_VERSION
    );
    ensure!(
        !state.boards.is_empty() && state.boards.len() <= MAX_BOARDS,
        "A workspace supports between 1 and 16 boards"
    );
    ensure!(
        state.next_board_id > 0 && state.next_board_id <= MAX_STABLE_ID,
        "Invalid next board ID"
    );
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    let mut window_ids = HashSet::new();
    let mut panel_ids = HashSet::new();
    for board in &state.boards {
        ensure!(
            board.id > 0 && board.id <= MAX_STABLE_ID,
            "Invalid board ID"
        );
        ensure!(ids.insert(board.id), "Duplicate board ID {}", board.id);
        ensure!(
            board.id < state.next_board_id,
            "Next board ID must exceed every saved board ID"
        );
        let name = validate_board_name(&board.name)?;
        ensure!(name == board.name, "Saved board names must be trimmed");
        ensure!(names.insert(name), "Duplicate board name {}", board.name);
        snapshot::validate(&board.workspace)?;
        ensure!(
            board
                .workspace
                .windows
                .iter()
                .filter(|window| window.id == 0)
                .count()
                == 1,
            "Board {} must contain exactly one main window",
            board.id
        );
        // The main window is shared by all boards. Floating-window and known
        // panel identities must remain distinct even while their board is not
        // materialized, so switching cannot reuse another board's native state.
        // Window and panel IDs remain separate namespaces for legacy files.
        for window in &board.workspace.windows {
            ensure!(
                window.id == 0 || window_ids.insert(window.id),
                "Duplicate floating window ID {} across boards",
                window.id
            );
            validate_global_panel_ids(&window.dock.center, &mut panel_ids)?;
            for dock in [
                &window.dock.left_dock,
                &window.dock.right_dock,
                &window.dock.bottom_dock,
            ]
            .into_iter()
            .flatten()
            {
                validate_global_panel_ids(dock.panel(), &mut panel_ids)?;
            }
        }
    }
    ensure!(
        ids.contains(&state.active_board),
        "Active board {} does not exist",
        state.active_board
    );
    ensure!(
        state.next_entity_id() <= MAX_STABLE_ID,
        "Panel/window ID allocation is exhausted"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
