//! Design-review choices are independent of business data and personal layouts.
//! Invalid, unreadable or externally changed files are never overwritten.
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, bail, ensure};
use gpui_kit::{App, Global};
use serde::{Deserialize, Serialize};

use crate::adapters::layout as storage;
use crate::gallery::foundation::{Family, Variant};

const VERSION: u32 = 1;
const MAX_BYTES: u64 = 64 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreferencesFile {
    version: u32,
    choices: BTreeMap<String, String>,
}

pub struct GalleryPreferences {
    pub choices: [Option<Variant>; 10],
    pub status: String,
    path: Option<PathBuf>,
    baseline: Option<Vec<u8>>,
    blocked: bool,
}
impl Global for GalleryPreferences {}

/// Tests pass false: no profile path is resolved, read or written.
pub fn init(persistent: bool, cx: &mut App) {
    cx.set_global(load(
        persistent.then(|| storage::data_dir().join("gallery-preferences.json")),
    ));
}

pub fn choose(family: Family, variant: Variant, cx: &mut App) {
    let state = cx.global_mut::<GalleryPreferences>();
    let slot = &mut state.choices[family.index()];
    *slot = if *slot == Some(variant) {
        None
    } else {
        Some(variant)
    };
    persist(state);
    cx.refresh_windows();
}

pub fn clear(cx: &mut App) {
    let state = cx.global_mut::<GalleryPreferences>();
    state.choices = [None; 10];
    persist(state);
    cx.refresh_windows();
}

/// Include every family, even when incomplete, so copying never invents a choice.
pub fn summary(cx: &App) -> String {
    describe(cx.global::<GalleryPreferences>())
}

fn describe(state: &GalleryPreferences) -> String {
    let count = state.choices.iter().flatten().count();
    let mut text = format!(
        "Tydaro 基础组件选择（{count}/10）\n{}\n\n| 组件 | 选择 |\n| --- | --- |\n",
        state.status
    );
    for family in Family::ALL {
        let choice = state.choices[family.index()]
            .map(|variant| format!("{} · {}", variant.letter(), variant.title()))
            .unwrap_or_else(|| "未选择".into());
        text.push_str(&format!("| {} | {choice} |\n", family.label()));
    }
    text
}

fn load(path: Option<PathBuf>) -> GalleryPreferences {
    let mut state = GalleryPreferences {
        choices: [None; 10],
        status: "测试会话 · 选择不写入磁盘".into(),
        path,
        baseline: None,
        blocked: false,
    };
    if let Some(path) = &state.path {
        match read_bytes(path) {
            Ok(bytes) => match decode(&bytes) {
                Ok(choices) => {
                    state.choices = choices;
                    state.baseline = Some(bytes);
                    state.status = "已恢复本机选择".into();
                }
                Err(error) => {
                    state.blocked = true;
                    state.status = format!("读取失败，原文件已保护；当前选择仅在内存：{error}");
                }
            },
            Err(error) if is_not_found(&error) => {
                state.status = "选择后自动保存到本机".into();
            }
            Err(error) => {
                state.blocked = true;
                state.status = format!("读取失败，原文件已保护；当前选择仅在内存：{error}");
            }
        }
    }
    state
}

fn persist(state: &mut GalleryPreferences) {
    let Some(path) = &state.path else {
        return;
    };
    if state.blocked {
        return;
    }
    match write_file(path, &state.choices, state.baseline.as_deref()) {
        Ok(bytes) => {
            state.baseline = Some(bytes);
            state.status = "选择已保存到本机".into();
        }
        Err(error) => {
            state.status = format!("保存失败，当前选择仅在内存；可复制结果：{error}");
        }
    }
}

fn decode(bytes: &[u8]) -> Result<[Option<Variant>; 10]> {
    let file: PreferencesFile = serde_json::from_slice(bytes).context("选择文件 JSON 无效")?;
    ensure!(
        file.version == VERSION,
        "不支持的选择文件版本 {}",
        file.version
    );
    let mut choices = [None; 10];
    for (key, value) in file.choices {
        let family = Family::ALL
            .into_iter()
            .find(|family| family.key() == key)
            .with_context(|| format!("未知组件 {key}"))?;
        let variant = Variant::ALL
            .into_iter()
            .find(|variant| variant.key() == value)
            .with_context(|| format!("未知方案 {value}"))?;
        choices[family.index()] = Some(variant);
    }
    Ok(choices)
}

fn encode(choices: &[Option<Variant>; 10]) -> Result<Vec<u8>> {
    let file = PreferencesFile {
        version: VERSION,
        choices: Family::ALL
            .into_iter()
            .filter_map(|family| {
                choices[family.index()].map(|variant| (family.key().into(), variant.key().into()))
            })
            .collect(),
    };
    Ok(serde_json::to_vec_pretty(&file)?)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path).context("无法读取选择文件")?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= MAX_BYTES, "选择文件超过 64 KiB");
    Ok(bytes)
}

fn is_not_found(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
}

fn suffix(path: &Path, tail: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(tail);
    PathBuf::from(name)
}

struct PendingFile(PathBuf);
impl PendingFile {
    fn create(path: &Path, bytes: &[u8]) -> Result<Self> {
        for _ in 0..128 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let candidate = suffix(path, &format!(".tmp-{}-{sequence}", std::process::id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    let pending = Self(candidate);
                    let result = file.write_all(bytes).and_then(|()| file.sync_all());
                    drop(file);
                    result?;
                    return Ok(pending);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error).context("无法创建选择临时文件"),
            }
        }
        bail!("无法分配选择临时文件")
    }

    fn replace(self, path: &Path) -> Result<()> {
        fs::rename(&self.0, path).context("无法原子替换选择文件")?;
        Ok(())
    }
}
impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Baseline comparison protects against observed external edits. It is not a
/// cross-process lock; callers in this process share one Global state.
fn write_file(
    path: &Path,
    choices: &[Option<Variant>; 10],
    baseline: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let bytes = encode(choices)?;
    ensure!(bytes.len() as u64 <= MAX_BYTES, "选择文件超过 64 KiB");
    let previous = match read_bytes(path) {
        Ok(bytes) => {
            decode(&bytes).context("现有选择文件异常，已停止覆盖")?;
            Some(bytes)
        }
        Err(error) if is_not_found(&error) => None,
        Err(error) => return Err(error),
    };
    ensure!(
        previous.as_deref() == baseline,
        "选择文件已被外部修改，已停止覆盖；请复制当前结果后重新打开"
    );
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let pending = PendingFile::create(path, &bytes)?;
    if let Some(previous) = previous {
        let backup = suffix(path, ".bak");
        PendingFile::create(&backup, &previous)?.replace(&backup)?;
    }
    pending.replace(path)?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(bytes)
}

#[cfg(test)]
mod tests;
