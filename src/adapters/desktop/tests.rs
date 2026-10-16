use super::*;
use crate::market::Symbol;
use crate::test_support::TestDirectory;
use tydaro_core::desktop::{DEMO_DATE, Draft, blank_plan, seed};

#[test]
fn roundtrip_keeps_independent_data_and_exact_previous_backup() {
    let directory = TestDirectory::new();
    let path = directory.file();
    let mut state = seed();
    let original = write_file(&path, &state, None).unwrap();
    state.monitoring = false;
    state.watch_groups[0].name = "独立观察组".into();
    state.research = serde_json::json!({"rows":[],"edited":true});
    state.next_id = 102;
    state.journal.day_mut("2026-10-15").drafts.push(Draft {
        plan: blank_plan("2026-10-15", "draft-101".into()),
        editing_id: None,
    });
    let saved = write_file(&path, &state, Some(&original)).unwrap();
    assert_eq!(fs::read(suffix(&path, ".bak")).unwrap(), original);
    assert_eq!(fs::read(&path).unwrap(), saved);
    let loaded = decode(&saved).unwrap();
    assert!(!loaded.monitoring);
    assert_eq!(loaded.watch_groups[0].name, "独立观察组");
    assert_eq!(
        loaded.journal.days["2026-10-15"].drafts[0].plan.date,
        "2026-10-15"
    );
    assert_eq!(loaded.research["edited"], true);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 2);
}

#[test]
fn damaged_unknown_or_external_data_is_never_replaced() {
    let directory = TestDirectory::new();
    let path = directory.file();
    let original = write_file(&path, &seed(), None).unwrap();
    for data in [b"{ broken".to_vec(), b"{\"version\":900}".to_vec()] {
        fs::write(&path, &data).unwrap();
        assert!(write_file(&path, &seed(), Some(&original)).is_err());
        assert_eq!(fs::read(&path).unwrap(), data);
        assert!(!suffix(&path, ".bak").exists());
    }
    let mut external = seed();
    external.muted = false;
    let bytes = serde_json::to_vec_pretty(&external).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert!(write_file(&path, &seed(), Some(&original)).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
    fs::remove_file(&path).unwrap();
    assert!(write_file(&path, &seed(), Some(&original)).is_err());
    assert!(!path.exists());
}

#[test]
fn invalid_new_state_leaves_valid_disk_and_backup_unchanged() {
    let directory = TestDirectory::new();
    let path = directory.file();
    let mut file = seed();
    let original = write_file(&path, &file, None).unwrap();
    file.watch_groups[0].symbols.push(Symbol::Yinli);
    assert!(write_file(&path, &file, Some(&original)).is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!suffix(&path, ".bak").exists());
}

#[test]
fn bounded_read_and_write_errors_preserve_current_memory_document() {
    let directory = TestDirectory::new();
    let path = directory.file();
    File::create(&path).unwrap().set_len(MAX_BYTES + 1).unwrap();
    assert!(read_bytes(&path).is_err());
    assert!(write_file(&path, &seed(), None).is_err());
    assert_eq!(fs::metadata(&path).unwrap().len(), MAX_BYTES + 1);
    let blocked = directory.0.join("not-a-directory");
    fs::write(&blocked, b"keep").unwrap();
    assert!(write_file(&blocked.join("desktop.json"), &seed(), None).is_err());
    assert_eq!(fs::read(blocked).unwrap(), b"keep");
}

#[test]
fn writer_serializes_baselines_and_rejects_late_old_revisions() {
    let directory = TestDirectory::new();
    let path = directory.file();
    let writer = WriteSession::shared(path.clone(), None);
    let mut first = seed();
    first.journal.day_mut(DEMO_DATE).note = "first".into();
    save(&writer, 1, &first).unwrap();
    let original = fs::read(&path).unwrap();
    let mut newer = first.clone();
    newer.journal.day_mut(DEMO_DATE).note = "newer close flush".into();
    save(&writer, 2, &newer).unwrap();
    assert_eq!(fs::read(suffix(&path, ".bak")).unwrap(), original);
    let latest = fs::read(&path).unwrap();
    save(&writer, 1, &first).unwrap();
    assert_eq!(fs::read(&path).unwrap(), latest);
    fs::write(&path, b"external edit").unwrap();
    assert!(save(&writer, 3, &first).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"external edit");
}
