use super::*;
use crate::model::{PanelKind, PanelModel};
use crate::panels::snapshot::ContentSnapshot;
use crate::workspace::boards::SavedBoard;
use crate::workspace::snapshot::{SavedPanel, SavedWindow};
use gpui_kit::{
    component::dock::{DockAreaState, DockPlacement, DockState, PanelInfo, PanelState},
    px,
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let number = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!("tydaro-storage-{}-{number}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn workspace(&self) -> PathBuf {
        self.0.join("workspace.json")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn panel(id: u64) -> PanelState {
    PanelState {
        panel_name: "tydaro.panel".into(),
        children: vec![],
        info: PanelInfo::Panel(
            serde_json::to_value(SavedPanel {
                model: PanelModel::new(id, PanelKind::Chart),
                content: serde_json::to_value(ContentSnapshot::new("100.00".into())).unwrap(),
            })
            .unwrap(),
        ),
    }
}

fn fixture() -> WorkspaceFile {
    WorkspaceFile {
        version: SCHEMA_VERSION,
        next_id: 10,
        paused: false,
        windows: vec![SavedWindow {
            id: 1,
            bounds: [-1920.0, 20.0, 1400.0, 900.0],
            locked: false,
            dock: DockAreaState {
                center: panel(2),
                ..Default::default()
            },
        }],
    }
}

fn board_fixture() -> BoardSetFile {
    let mut workspace = fixture();
    workspace.windows[0].id = 0;
    BoardSetFile::from_legacy(workspace)
}

fn preserved_files(dir: &TestDirectory) -> Vec<PathBuf> {
    fs::read_dir(&dir.0)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .contains(".corrupt-")
                .then_some(path)
        })
        .collect()
}

#[test]
fn old_panel_payload_defaults_name_and_custom_names_survive_roundtrip() {
    let dir = TestDirectory::new();
    let path = dir.workspace();
    let mut state = fixture();
    let PanelInfo::Panel(value) = &mut state.windows[0].dock.center.info else {
        panic!("fixture must contain a panel");
    };
    // An actual old payload has neither the new optional field nor a new
    // enum spelling. Keep Watchlist as the persisted component identity.
    value["model"]["kind"] = serde_json::json!("Watchlist");
    value["model"]
        .as_object_mut()
        .unwrap()
        .remove("custom_name");
    let old: SavedPanel = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(old.model.kind, PanelKind::Watchlist);
    assert_eq!(old.model.custom_name, None);
    assert_eq!(old.model.title(), "股票列表");
    assert!(
        serde_json::to_value(&old).unwrap()["model"]
            .get("custom_name")
            .is_none()
    );
    write(&path, &state).unwrap();
    let mut renamed = read(&path).unwrap();
    let PanelInfo::Panel(value) = &mut renamed.windows[0].dock.center.info else {
        panic!("saved fixture must contain a panel");
    };
    let mut saved: SavedPanel = serde_json::from_value(value.clone()).unwrap();
    saved.model.custom_name = PanelModel::normalize_name("  科技观察  ").unwrap();
    *value = serde_json::to_value(&saved).unwrap();
    write(&path, &renamed).unwrap();
    assert_eq!(
        serde_json::to_value(read(&path).unwrap()).unwrap(),
        serde_json::to_value(renamed).unwrap()
    );
    assert_eq!(saved.model.title(), "科技观察");
    saved.model.custom_name = PanelModel::normalize_name("   ").unwrap();
    assert_eq!(saved.model.title(), "股票列表");
}

#[test]
fn invalid_custom_names_cannot_replace_a_valid_layout() {
    let dir = TestDirectory::new();
    let path = dir.workspace();
    let mut state = fixture();
    write(&path, &state).unwrap();
    let original = fs::read(&path).unwrap();
    for name in [" ".to_owned(), "科技\n组".into(), "股".repeat(33)] {
        let PanelInfo::Panel(value) = &mut state.windows[0].dock.center.info else {
            panic!("fixture must contain a panel");
        };
        value["model"]["custom_name"] = serde_json::json!(name);
        assert!(write(&path, &state).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}

#[test]
fn legacy_migration_retains_all_content_and_backs_up_original_bytes() {
    let dir = TestDirectory::new();
    let path = dir.workspace();
    let mut legacy = board_fixture().boards.remove(0).workspace;
    let mut floating = legacy.windows[0].clone();
    // Legacy panel and window IDs occupy separate namespaces; migration
    // must still accept a floating window matching the main panel's ID.
    floating.id = 2;
    floating.locked = true;
    floating.dock.center = PanelState {
        panel_name: "future.extension".into(),
        children: vec![panel(7)],
        info: PanelInfo::Panel(serde_json::json!({"custom": ["preserve", 3]})),
    };
    legacy.windows.push(floating);
    legacy.paused = true;
    legacy.next_id = 50;
    // Noncanonical whitespace verifies that migration backs up the actual
    // prior file, not a fresh serialization of its decoded representation.
    let original = format!(" \n{}\n\n", serde_json::to_string(&legacy).unwrap()).into_bytes();
    fs::write(&path, &original).unwrap();
    let migrated = read_board_set(&path).unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(migrated.active_board, 1);
    assert_eq!(migrated.next_board_id, 2);
    assert_eq!(migrated.boards[0].name, "默认看板");
    assert_eq!(
        serde_json::to_value(&migrated.boards[0].workspace).unwrap(),
        serde_json::to_value(&legacy).unwrap()
    );
    write_board_set(&path, &migrated).unwrap();
    assert_eq!(fs::read(with_suffix(&path, ".bak")).unwrap(), original);
    assert_eq!(
        serde_json::to_value(read_board_set(&path).unwrap()).unwrap(),
        serde_json::to_value(migrated).unwrap()
    );
    assert!(
        read(&path).is_err(),
        "schema 2 must not be mistaken for a single-board template"
    );
}

#[test]
fn board_order_active_selection_and_independent_snapshots_roundtrip() {
    let dir = TestDirectory::new();
    let path = dir.workspace();
    let mut state = board_fixture();
    let mut other = state.boards[0].workspace.clone();
    other.paused = true;
    other.windows[0].locked = true;
    other.windows[0].dock.center = panel(21);
    other.next_id = 22;
    state.boards.insert(
        0,
        SavedBoard {
            id: 8,
            name: "竞价观察".into(),
            workspace: other,
        },
    );
    state.active_board = 8;
    state.next_board_id = 9;
    write_board_set(&path, &state).unwrap();
    assert_eq!(
        serde_json::to_value(read_board_set(&path).unwrap()).unwrap(),
        serde_json::to_value(&state).unwrap()
    );
    state.boards.swap(0, 1);
    state.active_board = 1;
    write_board_set(&path, &state).unwrap();
    let saved = read_board_set(&path).unwrap();
    assert_eq!(
        saved
            .boards
            .iter()
            .map(|board| board.id)
            .collect::<Vec<_>>(),
        vec![1, 8]
    );
    assert_eq!(saved.active_board, 1);
    assert_eq!(
        read_board_set(&with_suffix(&path, ".bak"))
            .unwrap()
            .active_board,
        8
    );
}

#[test]
fn corrupt_or_future_workspace_cannot_replace_the_last_good_board_backup() {
    let dir = TestDirectory::new();
    let path = dir.workspace();
    let state = board_fixture();
    write_board_set(&path, &state).unwrap();
    write_board_set(&path, &state).unwrap();
    let backup_path = with_suffix(&path, ".bak");
    let backup = fs::read(&backup_path).unwrap();
    let invalid_legacy = b"{\"version\":1,\"next_id\":5,\"paused\":false,\"windows\":[";
    let future = b"{\"version\":99,\"future_data\":\"retain me\"}";
    for original in [invalid_legacy.as_slice(), future.as_slice()] {
        fs::write(&path, original).unwrap();
        assert!(read_board_set(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        write_board_set(&path, &state).unwrap();
        assert_eq!(fs::read(&backup_path).unwrap(), backup);
        assert!(
            preserved_files(&dir)
                .iter()
                .any(|preserved| fs::read(preserved).unwrap() == original)
        );
    }
    assert_eq!(preserved_files(&dir).len(), 2);
}

#[test]
fn invalid_and_oversized_board_sets_leave_the_previous_file_unchanged() {
    let dir = TestDirectory::new();
    let path = dir.workspace();
    let state = board_fixture();
    write_board_set(&path, &state).unwrap();
    let original = fs::read(&path).unwrap();
    let mut invalid = state.clone();
    invalid.active_board = 999;
    assert!(write_board_set(&path, &invalid).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    invalid = state;
    invalid.boards[0].workspace.windows[0].dock.center = PanelState {
        panel_name: "future.large-panel".into(),
        children: vec![],
        info: PanelInfo::Panel(serde_json::json!({"data": "x".repeat(MAX_FILE_BYTES as usize)})),
    };
    assert!(write_board_set(&path, &invalid).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    fs::write(&path, vec![b' '; MAX_FILE_BYTES as usize + 1]).unwrap();
    assert!(read_board_set(&path).is_err());
}

#[test]
fn roundtrip_keeps_panel_content_and_previous_good_backup() {
    let dir = TestDirectory::new();
    let path = dir.workspace();
    let first = fixture();
    write(&path, &first).unwrap();
    assert_eq!(
        serde_json::to_value(read(&path).unwrap()).unwrap(),
        serde_json::to_value(&first).unwrap()
    );
    let mut second = first.clone();
    second.paused = true;
    write(&path, &second).unwrap();
    assert!(read(&path).unwrap().paused);
    assert!(!read(&with_suffix(&path, ".bak")).unwrap().paused);
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 2);
}

#[test]
fn corrupt_existing_file_is_preserved_byte_for_byte() {
    let dir = TestDirectory::new();
    let path = dir.workspace();
    let original = b"{interrupted layout\x00\xff";
    fs::write(&path, original).unwrap();
    assert!(read(&path).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    write(&path, &fixture()).unwrap();
    assert!(read(&path).is_ok());
    let preserved = fs::read_dir(&dir.0)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .contains(".corrupt-")
                .then_some(path)
        })
        .collect::<Vec<_>>();
    assert_eq!(preserved.len(), 1);
    assert_eq!(fs::read(&preserved[0]).unwrap(), original);
}

#[test]
fn rejected_write_leaves_existing_file_unchanged() {
    let dir = TestDirectory::new();
    let path = dir.workspace();
    let mut state = fixture();
    write(&path, &state).unwrap();
    let original = fs::read(&path).unwrap();
    state.version += 1;
    assert!(write(&path, &state).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    state.version = SCHEMA_VERSION;
    state.windows[0].bounds[2] = f32::NAN;
    assert!(write(&path, &state).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn corrupt_known_content_is_rejected_before_writing_or_restoring() {
    let dir = TestDirectory::new();
    let path = dir.workspace();
    let mut state = fixture();
    write(&path, &state).unwrap();
    let original = fs::read(&path).unwrap();
    let PanelInfo::Panel(payload) = &mut state.windows[0].dock.center.info else {
        panic!("fixture panel");
    };
    payload["content"]["chart"]["bars_visible"] = serde_json::json!("damaged");
    assert!(write(&path, &state).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);

    // A syntactically valid JSON document with damaged known content must
    // fail at the storage boundary, before any workspace view is created.
    let damaged = serde_json::to_vec(&state).unwrap();
    fs::write(&path, &damaged).unwrap();
    let error = read(&path).err().unwrap();
    assert!(format!("{error:#}").contains("Invalid content for panel"));
    assert_eq!(fs::read(&path).unwrap(), damaged);
}

#[test]
fn legacy_panels_without_content_keep_the_explicit_default_marker() {
    let mut state = fixture();
    let PanelInfo::Panel(payload) = &mut state.windows[0].dock.center.info else {
        panic!("fixture panel");
    };
    payload.as_object_mut().unwrap().remove("content");
    validate(&state).unwrap();
    let restored = decode(&serde_json::to_vec(&state).unwrap()).unwrap();
    let PanelInfo::Panel(payload) = &restored.windows[0].dock.center.info else {
        panic!("fixture panel");
    };
    let saved: SavedPanel = serde_json::from_value(payload.clone()).unwrap();
    assert!(saved.content.is_null());
}

#[test]
fn duplicate_ids_are_detected_across_windows_and_side_docks() {
    let mut state = fixture();
    state.windows.push(state.windows[0].clone());
    assert!(
        validate(&state)
            .unwrap_err()
            .to_string()
            .contains("window ID")
    );
    state.windows[1].id = 3;
    assert!(
        validate(&state)
            .unwrap_err()
            .to_string()
            .contains("panel ID")
    );
    state.windows.pop();
    for placement in [
        DockPlacement::Left,
        DockPlacement::Right,
        DockPlacement::Bottom,
    ] {
        let duplicate = Some(DockState::new(panel(2), placement, px(250.0), true));
        match placement {
            DockPlacement::Left => state.windows[0].dock.left_dock = duplicate,
            DockPlacement::Right => state.windows[0].dock.right_dock = duplicate,
            _ => state.windows[0].dock.bottom_dock = duplicate,
        }
        assert!(
            validate(&state)
                .unwrap_err()
                .to_string()
                .contains("panel ID")
        );
        state.windows[0].dock.left_dock = None;
        state.windows[0].dock.right_dock = None;
        state.windows[0].dock.bottom_dock = None;
    }
}

#[test]
fn unknown_panel_payload_survives_and_malformed_known_panel_is_rejected() {
    let dir = TestDirectory::new();
    let mut state = fixture();
    let unknown = PanelState {
        panel_name: "future.extension".into(),
        children: vec![panel(9)],
        info: PanelInfo::Panel(serde_json::json!({"extension_version": 7, "future": [1, 2, 3]})),
    };
    state.windows[0].dock.center = unknown.clone();
    write(&dir.workspace(), &state).unwrap();
    assert_eq!(
        read(&dir.workspace()).unwrap().windows[0].dock.center,
        unknown
    );
    state.windows[0].dock.center.panel_name = "tydaro.panel".into();
    assert!(validate(&state).is_err());
}

#[test]
fn limits_apply_across_the_whole_workspace() {
    let mut state = fixture();
    state.windows[0].dock.center = PanelState {
        panel_name: "TabPanel".into(),
        children: (1..=128).map(panel).collect(),
        info: PanelInfo::Tabs { active_index: 0 },
    };
    validate(&state).unwrap();
    state.windows[0].dock.center.children.push(panel(129));
    assert!(validate(&state).is_err());
    let mut state = fixture();
    state.windows = (0..17)
        .map(|id| SavedWindow {
            id,
            dock: DockAreaState {
                center: panel(id + 100),
                ..Default::default()
            },
            ..state.windows[0].clone()
        })
        .collect();
    assert!(validate(&state).is_err());
}

#[test]
fn non_finite_split_sizes_cannot_produce_unreadable_json() {
    let mut state = fixture();
    state.windows[0].dock.center = PanelState {
        panel_name: "StackPanel".into(),
        children: vec![panel(1)],
        info: PanelInfo::Stack {
            sizes: vec![px(f32::NAN)],
            axis: 0,
        },
    };
    assert!(validate(&state).is_err());
    state.windows[0].dock.center.info = PanelInfo::Stack {
        sizes: vec![px(0.0)],
        axis: 0,
    };
    validate(&state).unwrap();
}

#[test]
fn malformed_identifiers_cannot_overflow_runtime_allocation() {
    let mut state = fixture();
    state.next_id = u64::MAX;
    assert!(validate(&state).is_err());
    state.next_id = 10;
    state.windows[0].id = u64::MAX;
    assert!(validate(&state).is_err());
    state.windows[0].id = 0;
    state.windows[0].dock.center = panel(u64::MAX);
    assert!(validate(&state).is_err());
    let mut saved = SavedPanel {
        model: PanelModel::new(2, PanelKind::Chart),
        content: serde_json::Value::Null,
    };
    saved.model.link_group = 4;
    state.windows[0].dock.center.info = PanelInfo::Panel(serde_json::to_value(saved).unwrap());
    assert!(validate(&state).is_err());
}
