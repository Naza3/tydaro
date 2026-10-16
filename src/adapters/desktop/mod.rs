//! Versioned desktop document I/O; no GPUI, views or business mutation.
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tydaro_core::desktop::{DesktopDocument as DesktopFile, validate};
pub(crate) const MAX_BYTES: u64 = 8 * 1024 * 1024;
pub(crate) static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Shared by background saves and the final synchronous close flush. This lock
/// serializes filesystem commits, not UI mutation; only close waits for disk.
pub(crate) type SharedWriter = Arc<Mutex<WriteSession>>;
pub(crate) struct WriteSession {
    path: PathBuf,
    baseline: Option<Vec<u8>>,
    saved_revision: Option<u64>,
}
impl WriteSession {
    pub(crate) fn shared(path: PathBuf, baseline: Option<Vec<u8>>) -> SharedWriter {
        Arc::new(Mutex::new(Self {
            path,
            baseline,
            saved_revision: None,
        }))
    }
}
pub(crate) fn save(writer: &SharedWriter, revision: u64, file: &DesktopFile) -> Result<()> {
    let mut session = writer
        .lock()
        .map_err(|_| anyhow::anyhow!("保存任务状态不可用"))?;
    // A cancelled callback can leave blocking I/O running. A late old request
    // must never overwrite a newer close flush, even if it acquires the lock last.
    if session
        .saved_revision
        .is_some_and(|saved| revision <= saved)
    {
        return Ok(());
    }
    let bytes = write_file(&session.path, file, session.baseline.as_deref())?;
    session.baseline = Some(bytes);
    session.saved_revision = Some(revision);
    Ok(())
}

pub(crate) fn decode(bytes: &[u8]) -> Result<DesktopFile> {
    let file = serde_json::from_slice(bytes).context("桌面数据 JSON 无效")?;
    validate(&file)?;
    Ok(file)
}
pub(crate) fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path).context("无法读取桌面数据文件")?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= MAX_BYTES, "桌面数据文件超过 8 MiB");
    Ok(bytes)
}
pub(crate) fn is_not_found(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}
pub(crate) fn suffix(path: &Path, tail: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(tail);
    PathBuf::from(name)
}
struct PendingFile {
    path: PathBuf,
}
impl PendingFile {
    fn create(path: &Path, bytes: &[u8]) -> Result<Self> {
        for _ in 0..128 {
            let n = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let candidate = suffix(path, &format!(".tmp-{}-{n}", std::process::id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    let pending = Self { path: candidate };
                    file.write_all(bytes)?;
                    file.sync_all()?;
                    drop(file); // Windows replacement must not retain an open temporary handle.
                    return Ok(pending);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error).context("无法创建桌面数据临时文件"),
            }
        }
        bail!("无法分配桌面数据临时文件")
    }
    fn replace(self, path: &Path) -> Result<()> {
        fs::rename(&self.path, path).context("无法原子替换桌面数据")?;
        Ok(())
    }
}
impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Refuse unreadable or externally changed predecessors. A valid predecessor is
/// retained byte-for-byte in .bak before the same-directory atomic replacement.
pub(crate) fn write_file(
    path: &Path,
    file: &DesktopFile,
    baseline: Option<&[u8]>,
) -> Result<Vec<u8>> {
    validate(file)?;
    let bytes = serde_json::to_vec_pretty(file)?;
    ensure!(bytes.len() as u64 <= MAX_BYTES, "桌面数据超过 8 MiB");
    let previous = match read_bytes(path) {
        Ok(bytes) => {
            decode(&bytes).context("现有文件异常，已停止覆盖")?;
            Some(bytes)
        }
        Err(error) if is_not_found(&error) => None,
        Err(error) => return Err(error),
    };
    ensure!(
        previous.as_deref() == baseline,
        "文件已被外部修改，已停止覆盖；请保留当前内容后重新打开"
    );
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
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
