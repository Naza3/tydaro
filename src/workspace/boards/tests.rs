//! Unit regressions for this module.

use super::*;
use crate::model::{PanelKind, PanelModel};
use crate::workspace::snapshot::SavedWindow;
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
                content: serde_json::to_value(crate::panels::snapshot::ContentSnapshot::new(
                    "100.00".into(),
                ))
                .unwrap(),
            })
            .unwrap(),
        ),
    }
}

fn fixture() -> BoardSetFile {
    BoardSetFile::from_legacy(WorkspaceFile {
        version: snapshot::SCHEMA_VERSION,
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
