use super::*;

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tydaro-gallery-preferences-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self) -> PathBuf {
        self.0.join("gallery-preferences.json")
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn mixed_choices_restore_by_stable_key_and_clearing_preserves_backup() {
    let directory = TestDirectory::new();
    let path = directory.file();
    let mut state = load(Some(path.clone()));
    assert!(
        !path.exists(),
        "opening a gallery must not write empty choices"
    );
    state.choices[Family::Actions.index()] = Some(Variant::Context);
    state.choices[Family::List.index()] = Some(Variant::Calm);
    persist(&mut state);
    let original = fs::read(&path).unwrap();
    let document: serde_json::Value = serde_json::from_slice(&original).unwrap();
    assert_eq!(document["choices"]["actions"], "c");
    assert_eq!(document["choices"]["list"], "b");
    assert_eq!(load(Some(path.clone())).choices, state.choices);
    state.choices = [None; 10];
    persist(&mut state);
    assert_eq!(load(Some(path.clone())).choices, [None; 10]);
    assert_eq!(fs::read(suffix(&path, ".bak")).unwrap(), original);
}

#[test]
fn corrupt_unknown_and_oversized_files_are_protected_without_losing_drafts() {
    let directory = TestDirectory::new();
    let path = directory.file();
    for original in [
        b"broken".to_vec(),
        br#"{"version":2,"choices":{}}"#.to_vec(),
        br#"{"version":1,"choices":{"future":"a"}}"#.to_vec(),
        br#"{"version":1,"choices":{"actions":"future"}}"#.to_vec(),
        br#"{"version":1,"choices":{},"future":true}"#.to_vec(),
        vec![b' '; MAX_BYTES as usize + 1],
    ] {
        fs::write(&path, &original).unwrap();
        let mut state = load(Some(path.clone()));
        state.choices[Family::Input.index()] = Some(Variant::Context);
        persist(&mut state);
        assert!(state.blocked);
        assert!(state.status.contains("原文件已保护"));
        assert!(describe(&state).contains("文本输入 | C"));
        assert_eq!(fs::read(&path).unwrap(), original);
        assert!(!suffix(&path, ".bak").exists());
    }
}

#[test]
fn external_changes_and_deletion_block_overwrites_and_keep_in_memory_choices() {
    let directory = TestDirectory::new();
    let path = directory.file();
    let original = encode(&[None; 10]).unwrap();
    for replacement in [Some(original.clone()), Some(b"damaged".to_vec()), None] {
        let mut choices = [None; 10];
        choices[Family::Actions.index()] = Some(Variant::Precision);
        fs::write(&path, encode(&choices).unwrap()).unwrap();
        let mut state = load(Some(path.clone()));
        if let Some(bytes) = &replacement {
            fs::write(&path, bytes).unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        state.choices[Family::Actions.index()] = Some(Variant::Context);
        persist(&mut state);
        assert!(state.status.contains("保存失败"));
        assert_eq!(
            state.choices[Family::Actions.index()],
            Some(Variant::Context)
        );
        assert_eq!(fs::read(&path).ok(), replacement);
        assert!(!suffix(&path, ".bak").exists());
    }
}

#[test]
fn write_failure_retains_copyable_choices_and_memory_mode_has_no_path() {
    let directory = TestDirectory::new();
    let blocked_parent = directory.0.join("ordinary-file");
    let mut state = load(Some(blocked_parent.join("gallery-preferences.json")));
    fs::write(&blocked_parent, b"keep").unwrap();
    state.choices[Family::Tabs.index()] = Some(Variant::Calm);
    persist(&mut state);
    assert!(state.status.contains("保存失败"));
    assert!(describe(&state).contains("页签导航 | B"));
    assert_eq!(fs::read(&blocked_parent).unwrap(), b"keep");

    let mut memory = load(None);
    memory.choices[Family::Tabs.index()] = Some(Variant::Calm);
    persist(&mut memory);
    assert!(memory.path.is_none());
    assert!(memory.baseline.is_none());
    assert!(memory.status.contains("不写入磁盘"));
    assert!(describe(&memory).contains("（1/10）"));
}
