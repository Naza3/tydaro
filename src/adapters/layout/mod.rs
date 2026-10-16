//! Versioned workspace storage. Files are replaced only after the new contents
//! have been validated and synced; previous and unreadable layouts are retained.

use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, bail, ensure};

use crate::workspace::boards::BoardSetFile;

use crate::workspace::snapshot::{SCHEMA_VERSION, WorkspaceFile, validate};
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
static FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Platform configuration directory; tests and portable installations can set
/// `TYDARO_DATA_DIR` without changing a user's ordinary workspace.
pub fn data_dir() -> PathBuf {
    let configured = |key| {
        env::var_os(key)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    if let Some(path) = configured("TYDARO_DATA_DIR") {
        return path;
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(path) = configured("LOCALAPPDATA") {
            return path.join("Tydaro");
        }
        if let Some(path) = configured("USERPROFILE") {
            return path.join("AppData").join("Local").join("Tydaro");
        }
    }
    if let Some(path) = configured("XDG_CONFIG_HOME") {
        return path.join("tydaro");
    }
    if let Some(path) = configured("HOME") {
        return path.join(".config").join("tydaro");
    }
    // An explicit local fallback is preferable to selecting another user's
    // profile when a restricted launcher omits all profile variables.
    PathBuf::from(".tydaro")
}

fn decode(bytes: &[u8]) -> Result<WorkspaceFile> {
    let state: WorkspaceFile = serde_json::from_slice(bytes).context("Invalid workspace JSON")?;
    validate(&state)?;
    Ok(state)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path).with_context(|| format!("Cannot open {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_FILE_BYTES,
        "Workspace file exceeds 8 MiB"
    );
    Ok(bytes)
}

pub fn read(path: &Path) -> Result<WorkspaceFile> {
    decode(&read_bytes(path)?).with_context(|| format!("Cannot load {}", path.display()))
}

fn decode_board_set(bytes: &[u8]) -> Result<BoardSetFile> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).context("Invalid workspace JSON")?;
    let version = value.get("version").and_then(serde_json::Value::as_u64);
    let state = match version {
        Some(version) if version == u64::from(SCHEMA_VERSION) => {
            let legacy: WorkspaceFile =
                serde_json::from_value(value).context("Invalid legacy workspace")?;
            BoardSetFile::from_legacy(legacy)
        }
        Some(version) if version == u64::from(crate::workspace::boards::SCHEMA_VERSION) => {
            serde_json::from_value(value).context("Invalid board workspace")?
        }
        _ => bail!("Unsupported workspace schema {version:?}"),
    };
    crate::workspace::boards::validate(&state)?;
    Ok(state)
}

/// Read all boards, wrapping a schema 1 workspace in the default board without
/// changing its windows, content, identities or on-disk bytes.
pub fn read_board_set(path: &Path) -> Result<BoardSetFile> {
    decode_board_set(&read_bytes(path)?).with_context(|| format!("Cannot load {}", path.display()))
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// A same-directory reserved file. Dropping it removes an unfinished write.
struct TemporaryFile {
    path: PathBuf,
    file: File,
    keep: bool,
}

impl TemporaryFile {
    fn create(path: &Path, role: &str) -> Result<Self> {
        for _ in 0..128 {
            let sequence = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let candidate =
                with_suffix(path, &format!(".{role}-{}-{sequence}", std::process::id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(file) => {
                    return Ok(Self {
                        path: candidate,
                        file,
                        keep: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error).context("Cannot create workspace temporary file"),
            }
        }
        bail!("Could not reserve a unique workspace temporary file")
    }

    fn store(&mut self, bytes: &[u8]) -> Result<()> {
        self.file.write_all(bytes)?;
        self.file.sync_all()?;
        Ok(())
    }

    fn replace(mut self, path: &Path) -> Result<()> {
        // Rust's rename replaces existing files on both Unix and Windows. The
        // source stays on the same filesystem, so readers see a whole file.
        fs::rename(&self.path, path)
            .with_context(|| format!("Cannot replace {}", path.display()))?;
        self.keep = true;
        Ok(())
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Persist a validated workspace with atomic replacement. A valid previous
/// file is saved as `.bak`; invalid previous bytes are retained verbatim in a
/// unique `.corrupt-*` file, without replacing the last good backup.
pub fn write(path: &Path, state: &WorkspaceFile) -> Result<()> {
    validate(state)?;
    let bytes = serde_json::to_vec_pretty(state).context("Cannot serialize workspace")?;
    write_validated(path, &bytes, |previous| decode(previous).is_ok())
}

/// Save a board set using the same atomic replacement and recovery semantics as
/// a single-board template. A valid schema 1 predecessor remains byte-for-byte
/// available in `.bak` on the first schema 2 save.
pub fn write_board_set(path: &Path, state: &BoardSetFile) -> Result<()> {
    crate::workspace::boards::validate(state)?;
    let bytes = serde_json::to_vec_pretty(state).context("Cannot serialize board workspace")?;
    write_validated(path, &bytes, |previous| decode_board_set(previous).is_ok())
}

fn write_validated(
    path: &Path,
    bytes: &[u8],
    previous_is_valid: impl FnOnce(&[u8]) -> bool,
) -> Result<()> {
    ensure!(
        bytes.len() as u64 <= MAX_FILE_BYTES,
        "Workspace file exceeds 8 MiB"
    );
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("Cannot create {}", parent.display()))?;

    let mut pending = TemporaryFile::create(path, "tmp")?;
    pending.store(bytes)?;
    match read_bytes(path) {
        Ok(previous) => {
            if previous_is_valid(&previous) {
                let backup_path = with_suffix(path, ".bak");
                let mut backup = TemporaryFile::create(&backup_path, "tmp")?;
                backup.store(&previous)?;
                backup.replace(&backup_path)?;
            } else {
                let mut preserved = TemporaryFile::create(path, "corrupt")?;
                preserved.store(&previous)?;
                preserved.keep = true;
                eprintln!(
                    "Preserved unreadable workspace: {}",
                    preserved.path.display()
                );
            }
        }
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_some_and(|error| error.kind() == io::ErrorKind::NotFound) => {}
        Err(error) => return Err(error).context("Existing workspace was left unchanged"),
    }
    pending.replace(path)?;
    #[cfg(unix)]
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests;

pub(crate) mod writer;
