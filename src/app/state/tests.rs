use super::*;
use crate::test_support::TestDirectory;
use std::fs;

#[test]
fn loaded_corrupt_file_is_protected_and_save_failures_keep_drafts_in_memory() {
    let directory = TestDirectory::new();
    let path = directory.file();
    fs::write(&path, b"broken original").unwrap();
    let mut state = load_state(Some(path.clone()));
    state
        .core
        .execute(Command::SetNote {
            date: DEMO_DATE.into(),
            text: "new text".into(),
        })
        .unwrap();
    state.revision += 1;
    state.dirty = true;
    assert!(!persist_state(&mut state));
    assert!(state.save_status.contains("原文件已保护"));
    assert_eq!(fs::read(&path).unwrap(), b"broken original");
    assert_eq!(state.journal.days[DEMO_DATE].note, "new text");
    fs::remove_file(&path).unwrap();
    let mut state = load_state(Some(path.clone()));
    state.revision += 1;
    state.dirty = true;
    assert!(persist_state(&mut state));
    assert_eq!(state.save_status, "已保存到本机");
    fs::write(&path, b"externally damaged").unwrap();
    state
        .core
        .execute(Command::SetNote {
            date: DEMO_DATE.into(),
            text: "unsaved newer text".into(),
        })
        .unwrap();
    state.revision += 1;
    state.dirty = true;
    assert!(!persist_state(&mut state));
    assert!(state.save_status.contains("保存失败"));
    assert!(state.dirty);
    assert_eq!(state.journal.days[DEMO_DATE].note, "unsaved newer text");
    assert_eq!(fs::read(path).unwrap(), b"externally damaged");
}

#[test]
fn late_save_completions_cannot_mark_new_edits_saved() {
    let mut state = DesktopState::from_file(seed());
    state.revision = 2;
    state.dirty = true;
    state.save_status = "正在保存…".into();
    complete_save(&mut state, 1, Ok(()));
    assert!(state.dirty);
    assert_eq!(state.save_status, "正在保存…");
    complete_save(&mut state, 1, Err(anyhow::anyhow!("late failure")));
    assert_eq!(state.save_status, "正在保存…");
    complete_save(&mut state, 2, Ok(()));
    assert!(!state.dirty);
}
