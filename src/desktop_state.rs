//! GPUI adapter for the independently validated business aggregate.
//! Storage retains the existing desktop.json wire format and failure protections.
use crate::{
    desktop_repository::{
        self as repository, SharedWriter, WriteSession, decode, is_not_found, read_bytes,
    },
    storage,
};
use anyhow::Result;
use gpui_kit::{App, Global, Task};
use std::{collections::BTreeMap, ops::Deref, path::PathBuf, time::Duration};
pub use tydaro_core::desktop::{
    Command, CommandResult, DEMO_DATE, DesktopEvent, Draft, PageId, Plan, SourceContext,
    WatchAlert, WatchGroup, WatchRule, date_valid, valid_date,
};
use tydaro_core::desktop::{Desktop, DesktopDocument as DesktopFile, seed};

pub struct DesktopState {
    core: Desktop,
    pub save_status: String,
    operation_error: Option<String>,
    pending_input_errors: BTreeMap<String, String>,
    writer: Option<SharedWriter>,
    save_task: Option<Task<()>>,
    blocked: bool,
    revision: u64,
    dirty: bool,
}
impl Global for DesktopState {}
impl Deref for DesktopState {
    type Target = Desktop;
    fn deref(&self) -> &Desktop {
        &self.core
    }
}
impl DesktopState {
    fn snapshot(&self) -> DesktopFile {
        self.core.snapshot()
    }
    fn from_file(file: DesktopFile) -> Self {
        Self {
            core: Desktop::try_from(file).expect("validated desktop document"),
            save_status: "本地演示数据".into(),
            operation_error: None,
            pending_input_errors: BTreeMap::new(),
            writer: None,
            save_task: None,
            blocked: false,
            revision: 0,
            dirty: false,
        }
    }
    pub fn status_message(&self) -> &str {
        self.pending_input_errors
            .values()
            .next()
            .map(String::as_str)
            .or(self.operation_error.as_deref())
            .unwrap_or(&self.save_status)
    }
}

/// Views retain invalid edits locally. Track only their error/identity here so
/// closing the application cannot silently discard text rejected by the core.
pub(crate) fn report_pending_input(cx: &mut App, key: impl Into<String>, error: Option<String>) {
    let state = cx.global_mut::<DesktopState>();
    let key = key.into();
    if let Some(error) = error {
        state
            .pending_input_errors
            .insert(key, format!("输入尚未保存：{error}"));
    } else {
        state.pending_input_errors.remove(&key);
        state.operation_error = None;
    }
    cx.refresh_windows();
}
/// `persistent = false` is used by smoke tests and never opens a user's profile.
pub fn init(persistent: bool, cx: &mut App) {
    cx.set_global(load_state(
        persistent.then(|| storage::data_dir().join("desktop.json")),
    ));
}

fn load_state(path: Option<PathBuf>) -> DesktopState {
    let mut state = DesktopState::from_file(seed());
    if let Some(path) = path {
        let mut baseline = None;
        match read_bytes(&path) {
            Ok(bytes) => match decode(&bytes) {
                Ok(saved) => {
                    state = DesktopState::from_file(saved);
                    baseline = Some(bytes);
                    state.save_status = "已恢复本地数据".into();
                }
                Err(error) => {
                    state.blocked = true;
                    state.save_status = format!("读取失败，原文件已保护；修改仅在内存：{error}");
                }
            },
            Err(error) if is_not_found(&error) => {
                state.save_status = "新工作区 · 修改后自动保存".into()
            }
            Err(error) => {
                state.blocked = true;
                state.save_status = format!("读取失败，原文件已保护；修改仅在内存：{error}");
            }
        }
        state.writer = Some(WriteSession::shared(path, baseline));
    } else {
        state.save_status = "测试会话 · 不写入磁盘".into();
    }
    state
}

/// Execute one validated command; rejected commands never advance the revision.
pub fn dispatch(cx: &mut App, command: Command) -> Result<CommandResult> {
    let state = cx.global_mut::<DesktopState>();
    let result = match state.core.execute(command) {
        Ok(result) => result,
        Err(error) => {
            state.operation_error = Some(format!("修改未应用：{error}"));
            cx.refresh_windows();
            return Err(error);
        }
    };
    state.operation_error = None;
    state.revision = state
        .revision
        .checked_add(1)
        .expect("desktop revision exhausted");
    state.dirty = true;
    state.save_task = None;
    if state.writer.is_none() {
        state.dirty = false;
    } else if !state.blocked {
        state.save_status = "正在保存…".into();
        schedule_save(cx);
    }
    cx.refresh_windows();
    Ok(result)
}

fn schedule_save(cx: &mut App) {
    let task = cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(700))
            .await;
        let request = cx.update(|cx| {
            let state = cx.global::<DesktopState>();
            if !state.dirty || state.blocked {
                return None;
            }
            Some((state.writer.clone()?, state.revision, state.snapshot()))
        });
        let Some((writer, revision, snapshot)) = request else {
            return;
        };
        let result = cx
            .background_executor()
            .spawn(async move { repository::save(&writer, revision, &snapshot) })
            .await;
        cx.update(|cx| {
            complete_save(cx.global_mut::<DesktopState>(), revision, result);
            cx.refresh_windows();
        });
    });
    cx.global_mut::<DesktopState>().save_task = Some(task);
}

fn complete_save(state: &mut DesktopState, revision: u64, result: Result<()>) {
    if state.revision != revision {
        return;
    }
    match result {
        Ok(()) => {
            state.dirty = false;
            state.save_status = "已保存到本机".into();
        }
        Err(error) => state.save_status = format!("保存失败，修改仅在内存：{error}"),
    }
}

/// Closing is an explicit durability boundary: cancel the debounce, wait for
/// any already-running write, then persist the latest revision before exiting.
pub fn save_now(cx: &mut App) -> bool {
    let saved = persist_state(cx.global_mut::<DesktopState>());
    let has_pending_input = !cx.global::<DesktopState>().pending_input_errors.is_empty();
    cx.refresh_windows();
    saved && !has_pending_input
}

fn persist_state(state: &mut DesktopState) -> bool {
    state.save_task = None;
    if !state.dirty {
        return !state.blocked;
    }
    let Some(writer) = state.writer.clone() else {
        state.dirty = false;
        return true;
    };
    if state.blocked {
        return false;
    }
    let result = repository::save(&writer, state.revision, &state.snapshot());
    let saved = result.is_ok();
    complete_save(state, state.revision, result);
    saved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop_repository::{MAX_BYTES, TEMP_SEQUENCE, suffix, write_file};
    use crate::market::Symbol;
    use std::{
        fs::{self, File},
        sync::atomic::Ordering,
    };
    use tydaro_core::desktop::{Journal, blank_plan, validate};

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            let n = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("tydaro-desktop-{}-{n}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn file(&self) -> PathBuf {
            self.0.join("desktop.json")
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn complete_sell(date: &str, id: &str) -> Plan {
        let mut plan = blank_plan(date, id.into());
        plan.sector = "测试板块".into();
        plan.sell_method = "跌破支撑".into();
        plan.exit = "收盘低于前日低点".into();
        plan
    }

    #[test]
    fn dates_reject_impossible_or_non_iso_days() {
        for date in ["2024-02-29", "2026-10-14", "2000-02-29"] {
            assert!(date_valid(date));
        }
        for date in [
            "2026-02-29",
            "1900-02-29",
            "2026-13-01",
            "2026-04-31",
            "2026-00-00",
            "2026-+1-01",
            "2026-1-01",
            "2026年10月14日",
        ] {
            assert!(!date_valid(date), "{date}");
        }
    }

    #[test]
    fn sell_only_plan_is_complete_but_partial_buy_side_stays_a_draft() {
        let mut plan = complete_sell(DEMO_DATE, "new-plan");
        assert!(plan.validate_complete().is_ok());
        plan.avoid = "不追高".into();
        assert!(plan.validate_complete().is_err());
        plan.method = "回踩确认".into();
        plan.entry = "量能恢复后确认".into();
        assert!(plan.validate_complete().is_ok());
        plan.sell_method.clear();
        assert!(plan.validate_complete().is_err());
    }

    #[test]
    fn editing_updates_identity_and_missing_target_never_appends() {
        let mut journal = Journal::default();
        journal
            .save_plan(complete_sell(DEMO_DATE, "original"), None)
            .unwrap();
        let mut edited = complete_sell(DEMO_DATE, "editor-draft");
        edited.exit = "新的触发条件".into();
        journal.save_plan(edited.clone(), Some("original")).unwrap();
        let day = &journal.days[DEMO_DATE];
        assert_eq!(day.plans.len(), 1);
        assert_eq!(day.plans[0].id, "original");
        assert_eq!(day.plans[0].exit, "新的触发条件");
        assert!(journal.save_plan(edited.clone(), Some("deleted")).is_err());
        edited.date = "2026-10-13".into();
        assert!(journal.save_plan(edited, Some("original")).is_err());
        assert_eq!(journal.days.len(), 1);
    }

    #[test]
    fn references_create_independent_drafts_without_touching_existing_plans() {
        let mut state = DesktopState::from_file(seed());
        let before = state.journal.days[DEMO_DATE].plans.len();
        let context = SourceContext {
            date: "2026-10-13".into(),
            sector: "文化传媒".into(),
            symbols: vec![Symbol::Yinli, Symbol::Yinli],
            reason: "资料依据".into(),
            origin: PageId::Theme,
        };
        let first = state
            .core
            .execute(Command::CreateDraft {
                context: context.clone(),
                date: DEMO_DATE.into(),
            })
            .unwrap()
            .into_id()
            .unwrap();
        let second = state
            .core
            .execute(Command::CreateDraft {
                context: context.clone(),
                date: DEMO_DATE.into(),
            })
            .unwrap()
            .into_id()
            .unwrap();
        assert_ne!(first, second);
        let day = &state.journal.days[DEMO_DATE];
        assert_eq!(day.plans.len(), before);
        assert_eq!(day.drafts.len(), 2);
        assert_eq!(day.drafts[0].plan.id, first);
        assert_eq!(day.drafts[0].plan.symbols, vec![Symbol::Yinli]);
        assert!(day.drafts[0].plan.source.contains("2026-10-13"));
        assert_eq!(day.drafts[0].plan.date, DEMO_DATE);
        assert!(day.drafts[0].plan.validate_complete().is_err());
        validate(&state.snapshot()).unwrap(); // Partial sides are valid stored drafts.
    }

    #[test]
    fn watch_append_is_deduplicated_and_groups_remain_independent() {
        let mut state = DesktopState::from_file(seed());
        let other = state.watch_groups[1].symbols.clone();
        assert_eq!(
            state
                .core
                .execute(Command::AddToWatch {
                    group_id: "media".into(),
                    symbols: vec![Symbol::Yinli, Symbol::Catl, Symbol::Catl]
                })
                .unwrap()
                .into_count()
                .unwrap(),
            1
        );
        assert_eq!(state.watch_groups[0].symbols.len(), 4);
        assert_eq!(state.watch_groups[1].symbols, other);
        assert!(
            state
                .core
                .execute(Command::AddToWatch {
                    group_id: "deleted".into(),
                    symbols: vec![Symbol::Yinli]
                })
                .is_err()
        );
    }

    #[test]
    fn recent_returns_select_dates_before_sorting_profit() {
        let journal = seed().journal;
        let recent = journal.recent_buys();
        assert_eq!(recent.len(), 10);
        assert_eq!(recent[0].stock, "引力传媒");
        assert!(!recent.iter().any(|buy| buy.date == "2026-09-01"));
        assert!(recent.iter().any(|buy| buy.profit == 10.0));
        assert_eq!(recent.iter().filter(|buy| buy.profit > 10.0).count(), 1);
    }

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
    fn file_validation_rejects_duplicate_identity_and_cross_date_plan() {
        let mut file = seed();
        validate(&file).unwrap();
        let duplicate = file.watch_groups[0].clone();
        file.watch_groups.push(duplicate);
        assert!(validate(&file).is_err());
        file.watch_groups.pop();
        file.journal.day_mut(DEMO_DATE).plans[0].date = "2026-10-13".into();
        assert!(validate(&file).is_err());
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

    #[test]
    fn writer_serializes_baselines_and_rejects_late_old_revisions() {
        let directory = TestDirectory::new();
        let path = directory.file();
        let writer = WriteSession::shared(path.clone(), None);
        let mut first = seed();
        first.journal.day_mut(DEMO_DATE).note = "first".into();
        repository::save(&writer, 1, &first).unwrap();
        let original = fs::read(&path).unwrap();
        let mut newer = first.clone();
        newer.journal.day_mut(DEMO_DATE).note = "newer close flush".into();
        repository::save(&writer, 2, &newer).unwrap();
        assert_eq!(fs::read(suffix(&path, ".bak")).unwrap(), original);
        let latest = fs::read(&path).unwrap();
        repository::save(&writer, 1, &first).unwrap();
        assert_eq!(fs::read(&path).unwrap(), latest);
        fs::write(&path, b"external edit").unwrap();
        assert!(repository::save(&writer, 3, &first).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"external edit");
    }
}
