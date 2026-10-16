//! GPUI adapter for the independently validated business aggregate.
//! Storage retains the existing desktop.json wire format and failure protections.
use crate::adapters::desktop::{
    self as repository, SharedWriter, WriteSession, decode, is_not_found, read_bytes,
};
use crate::adapters::layout as storage;
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
mod tests;
