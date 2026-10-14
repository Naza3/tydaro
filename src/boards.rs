//! Named, ordered workspaces. A board keeps its complete window and panel tree;
//! only the selected board is materialized into native windows at runtime.

use std::collections::HashSet;

use anyhow::{Result, ensure};
use gpui_kit::component::dock::{PanelInfo, PanelState};
use serde::{Deserialize, Serialize};

use crate::storage::{self, MAX_STABLE_ID, SavedPanel, WorkspaceFile};

pub const SCHEMA_VERSION: u32 = 2;
pub const MAX_BOARDS: usize = 16;
const MAX_NAME_CHARACTERS: usize = 24;

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedBoard {
    pub id: u64,
    pub name: String,
    pub workspace: WorkspaceFile,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BoardSetFile {
    pub version: u32,
    pub active_board: u64,
    pub next_board_id: u64,
    pub boards: Vec<SavedBoard>,
}

impl BoardSetFile {
    /// The schema 1 payload is retained whole, including floating windows,
    /// unknown panels and saved input state.
    pub fn from_legacy(workspace: WorkspaceFile) -> Self {
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
    pub fn next_entity_id(&self) -> u64 {
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
pub fn validate_board_name(name: &str) -> Result<String> {
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

pub fn validate(state: &BoardSetFile) -> Result<()> {
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
        storage::validate(&board.workspace)?;
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
mod tests {
    use super::*;
    use crate::{
        model::{PanelKind, PanelModel},
        storage::SavedWindow,
    };
    use gpui_kit::{
        component::dock::{DockAreaState, DockPlacement, DockState},
        px,
    };

    fn panel(id: u64) -> PanelState {
        PanelState {
            panel_name: "tydaro.panel".into(),
            children: vec![],
            info: PanelInfo::Panel(
                serde_json::to_value(SavedPanel {
                    model: PanelModel::new(id, PanelKind::Chart),
                    content: serde_json::json!({"viewport": {"zoom": 2.0}}),
                })
                .unwrap(),
            ),
        }
    }

    fn fixture() -> BoardSetFile {
        BoardSetFile::from_legacy(WorkspaceFile {
            version: storage::SCHEMA_VERSION,
            next_id: 5,
            paused: false,
            windows: vec![SavedWindow {
                id: 0,
                bounds: [0.0, 0.0, 1200.0, 800.0],
                locked: false,
                dock: DockAreaState {
                    center: panel(2),
                    ..Default::default()
                },
            }],
        })
    }

    #[test]
    fn names_trim_whitespace_and_count_unicode_characters() {
        assert_eq!(validate_board_name("  超短看盘  ").unwrap(), "超短看盘");
        assert!(validate_board_name(&"盘".repeat(24)).is_ok());
        assert!(validate_board_name(&"盘".repeat(25)).is_err());
        for name in ["", " \u{3000} ", "看\n板", "名称\0", "\t看板"] {
            assert!(validate_board_name(name).is_err(), "accepted {name:?}");
        }
    }

    #[test]
    fn invalid_selection_duplicate_boards_and_non_monotonic_ids_are_rejected() {
        let original = fixture();
        validate(&original).unwrap();
        let mut state = original.clone();
        state.active_board = 2;
        assert!(validate(&state).is_err());
        state = original.clone();
        state.next_board_id = 1;
        assert!(validate(&state).is_err());
        state = original.clone();
        state.boards[0].id = 0;
        assert!(validate(&state).is_err());
        state = original.clone();
        state.next_board_id = u64::MAX;
        assert!(validate(&state).is_err());
        state = original.clone();
        state.boards[0].id = MAX_STABLE_ID;
        state.next_board_id = MAX_STABLE_ID;
        assert!(validate(&state).is_err());
        state = original.clone();
        state.boards.push(state.boards[0].clone());
        state.boards[1].name = "观察".into();
        assert!(
            validate(&state)
                .unwrap_err()
                .to_string()
                .contains("Duplicate board ID")
        );
        state.boards[1].id = 2;
        state.next_board_id = 3;
        state.boards[1].name = state.boards[0].name.clone();
        assert!(
            validate(&state)
                .unwrap_err()
                .to_string()
                .contains("Duplicate board name")
        );
        state.boards[1].name = " 默认看板 ".into();
        assert!(validate(&state).is_err());
    }

    #[test]
    fn every_board_needs_one_main_window_and_the_board_count_is_bounded() {
        let mut state = fixture();
        state.boards[0].workspace.windows[0].id = 4;
        assert!(validate(&state).is_err());
        state = fixture();
        let mut duplicate_main = state.boards[0].workspace.windows[0].clone();
        duplicate_main.dock.center = panel(3);
        state.boards[0].workspace.windows.push(duplicate_main);
        assert!(validate(&state).is_err());
        state = fixture();
        let board = state.boards[0].clone();
        state.boards = (1..=16)
            .map(|id| {
                let mut workspace = board.workspace.clone();
                workspace.windows[0].dock.center = panel(id + 100);
                SavedBoard {
                    id,
                    name: format!("看板{id}"),
                    workspace,
                }
            })
            .collect();
        state.next_board_id = 18;
        validate(&state).unwrap();
        state.boards.push(SavedBoard {
            id: 17,
            name: "看板17".into(),
            ..board
        });
        assert!(validate(&state).is_err());
        state.boards.clear();
        assert!(validate(&state).is_err());
    }

    #[test]
    fn allocation_scans_inactive_boards_floating_windows_and_hidden_side_tabs() {
        let mut state = fixture();
        let mut inactive = state.boards[0].clone();
        inactive.id = 2;
        inactive.name = "观察".into();
        inactive.workspace.next_id = 40;
        inactive.workspace.windows[0].dock.center = panel(3);
        let mut floating = inactive.workspace.windows[0].clone();
        floating.id = 50;
        floating.dock.center = panel(51);
        floating.dock.left_dock = Some(DockState::new(
            PanelState {
                panel_name: "TabPanel".into(),
                children: vec![panel(60), panel(70)],
                info: PanelInfo::Tabs { active_index: 0 },
            },
            DockPlacement::Left,
            px(200.0),
            true,
        ));
        inactive.workspace.windows.push(floating);
        state.boards.push(inactive);
        state.next_board_id = 3;
        validate(&state).unwrap();
        assert_eq!(state.next_entity_id(), 71);
        state.boards[0].workspace.next_id = 100;
        assert_eq!(state.next_entity_id(), 100);
        state.boards[0].workspace.windows[0].dock.center = panel(MAX_STABLE_ID);
        assert!(
            validate(&state).is_err(),
            "exhausted IDs must not be reused"
        );
    }

    #[test]
    fn inactive_board_panel_and_floating_ids_cannot_reuse_another_boards_identity() {
        let mut state = fixture();
        let mut floating = state.boards[0].workspace.windows[0].clone();
        floating.id = 9;
        floating.dock.center = panel(10);
        state.boards[0].workspace.windows.push(floating.clone());
        let mut other = state.boards[0].clone();
        other.id = 2;
        other.name = "观察".into();
        other.workspace.windows.truncate(1);
        other.workspace.windows[0].dock.center = panel(3);
        // A collision hidden in an inactive side-dock tab must also be caught.
        other.workspace.windows[0].dock.right_dock = Some(DockState::new(
            PanelState {
                panel_name: "TabPanel".into(),
                children: vec![panel(4), panel(10)],
                info: PanelInfo::Tabs { active_index: 0 },
            },
            DockPlacement::Right,
            px(200.0),
            false,
        ));
        state.boards.push(other);
        state.next_board_id = 3;
        assert!(
            validate(&state)
                .unwrap_err()
                .to_string()
                .contains("Duplicate panel ID 10 across boards")
        );
        state.boards[1].workspace.windows[0].dock.right_dock = None;
        floating.dock.center = panel(11);
        state.boards[1].workspace.windows.push(floating);
        assert!(
            validate(&state)
                .unwrap_err()
                .to_string()
                .contains("Duplicate floating window ID 9 across boards")
        );
        // A panel and a window may share an ID: legacy files used separate
        // namespaces. Both boards may also keep their main window ID 0.
        state.boards[1].workspace.windows[1].id = 2;
        validate(&state).unwrap();
    }
}
