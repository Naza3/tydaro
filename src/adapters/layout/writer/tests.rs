use super::*;
use crate::workspace::snapshot::{SavedWindow, WorkspaceFile};
use gpui_kit::component::dock::{DockAreaState, PanelInfo, PanelState};
use std::{
    fs,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("tydaro-layout-save-{}-{id}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("workspace.json")
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn snapshot(name: &str) -> BoardSetFile {
    let mut state = BoardSetFile::from_legacy(WorkspaceFile {
        version: crate::workspace::snapshot::SCHEMA_VERSION,
        next_id: 1,
        paused: false,
        windows: vec![SavedWindow {
            id: 0,
            bounds: [0., 0., 1024., 768.],
            locked: false,
            dock: DockAreaState {
                center: PanelState {
                    panel_name: "TabPanel".into(),
                    children: vec![],
                    info: PanelInfo::Tabs { active_index: 0 },
                },
                ..Default::default()
            },
        }],
    });
    state.boards[0].name = name.into();
    state
}

#[test]
fn late_background_write_cannot_replace_newer_close_flush() {
    let directory = TestDirectory::new();
    let writer = WriteSession::shared(directory.file());
    let delayed_writer = writer.clone();
    let (start, wait) = mpsc::channel();
    let background = thread::spawn(move || {
        wait.recv().unwrap();
        save(&delayed_writer, 1, &snapshot("旧布局"))
    });
    save(&writer, 2, &snapshot("退出时的最新布局")).unwrap();
    start.send(()).unwrap();
    background.join().unwrap().unwrap();
    let restored = storage::read_board_set(&directory.file()).unwrap();
    assert_eq!(restored.boards[0].name, "退出时的最新布局");
    assert!(!directory.0.join("workspace.json.bak").exists());
}

#[test]
fn failed_write_does_not_advance_revision_and_can_retry() {
    let directory = TestDirectory::new();
    let writer = WriteSession::shared(directory.file());
    let mut invalid = snapshot("坏布局");
    invalid.boards.clear();
    assert!(save(&writer, 4, &invalid).is_err());
    assert!(!directory.file().exists());
    save(&writer, 4, &snapshot("重试成功")).unwrap();
    assert_eq!(
        storage::read_board_set(&directory.file()).unwrap().boards[0].name,
        "重试成功"
    );
}

#[test]
fn stale_completion_cannot_acknowledge_new_live_changes_or_new_flush() {
    let mut state = SaveRevision::default();
    let first = state.begin();
    state.changed();
    assert_eq!(state.finish(first), Some(false));
    let second = state.begin();
    let flush = state.begin();
    assert!(flush > second);
    assert_eq!(state.finish(second), None);
    assert_eq!(state.finish(flush), Some(true));
    assert_eq!(state.finish(first), None);
}
