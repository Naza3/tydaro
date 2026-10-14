//! Versioned workspace storage. Files are replaced only after the new contents
//! have been validated and synced; previous and unreadable layouts are retained.

use std::{
    collections::HashSet,
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, bail, ensure};
use gpui_kit::component::dock::{DockAreaState, PanelInfo, PanelState};
use serde::{Deserialize, Serialize};

use crate::model::PanelModel;

pub const SCHEMA_VERSION: u32 = 1;
const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_STABLE_ID: u64 = u64::MAX / 2;
static FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedPanel {
    pub model: PanelModel,
    #[serde(default)]
    pub content: serde_json::Value,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedWindow {
    pub id: u64,
    pub bounds: [f32; 4],
    #[serde(default)]
    pub locked: bool,
    pub dock: DockAreaState,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub version: u32,
    pub next_id: u64,
    pub paused: bool,
    pub windows: Vec<SavedWindow>,
}

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

pub fn validate(state: &WorkspaceFile) -> Result<()> {
    ensure!(
        state.version == SCHEMA_VERSION,
        "Unsupported workspace schema {} (expected {})",
        state.version,
        SCHEMA_VERSION
    );
    ensure!(
        state.windows.len() <= 16,
        "A workspace supports at most 16 windows"
    );
    ensure!(
        state.next_id <= MAX_STABLE_ID,
        "Invalid next panel/window ID"
    );
    let mut window_ids = HashSet::new();
    let mut panel_ids = HashSet::new();
    let mut panel_count = 0;
    for window in &state.windows {
        ensure!(window.id <= MAX_STABLE_ID, "Invalid window ID");
        ensure!(
            window_ids.insert(window.id),
            "Duplicate window ID {}",
            window.id
        );
        ensure!(
            window.bounds.iter().all(|value| value.is_finite())
                && window.bounds[2] > 0.0
                && window.bounds[3] > 0.0,
            "Window {} has invalid bounds",
            window.id
        );
        validate_panel(&window.dock.center, &mut panel_ids, &mut panel_count, 0)?;
        for dock in [
            &window.dock.left_dock,
            &window.dock.right_dock,
            &window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            let size = f32::from(dock.size());
            ensure!(size.is_finite() && size >= 0.0, "Invalid side dock size");
            validate_panel(dock.panel(), &mut panel_ids, &mut panel_count, 0)?;
        }
    }
    Ok(())
}

fn validate_panel(
    panel: &PanelState,
    ids: &mut HashSet<u64>,
    count: &mut usize,
    depth: usize,
) -> Result<()> {
    ensure!(depth <= 64, "Workspace panel nesting exceeds 64 levels");
    if let PanelInfo::Stack { sizes, axis } = &panel.info {
        ensure!(*axis <= 1, "Invalid split axis");
        ensure!(
            sizes.iter().all(|size| {
                let size = f32::from(*size);
                size.is_finite() && size >= 0.0
            }),
            "Invalid split size"
        );
    }
    if matches!(panel.info, PanelInfo::Panel(_)) {
        *count += 1;
        ensure!(*count <= 128, "A workspace supports at most 128 panels");
    }
    if panel.panel_name == "tydaro.panel" {
        let PanelInfo::Panel(payload) = &panel.info else {
            bail!("A tydaro.panel must contain a saved panel payload");
        };
        let saved: SavedPanel = serde_json::from_value(payload.clone())
            .context("Invalid saved tydaro.panel payload")?;
        ensure!(saved.model.id <= MAX_STABLE_ID, "Invalid panel ID");
        ensure!(saved.model.link_group <= 3, "Invalid panel link group");
        ensure!(
            ids.insert(saved.model.id),
            "Duplicate panel ID {}",
            saved.model.id
        );
    }
    // Unknown panel names and their JSON payloads remain untouched for future
    // versions/plugins; their children still participate in identity checks.
    for child in &panel.children {
        validate_panel(child, ids, count, depth + 1)?;
    }
    Ok(())
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
    pending.store(&bytes)?;
    match read_bytes(path) {
        Ok(previous) => {
            if decode(&previous).is_ok() {
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
mod tests {
    use super::*;
    use crate::model::PanelKind;
    use gpui_kit::{
        component::dock::{DockPlacement, DockState},
        px,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let number = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path =
                env::temp_dir().join(format!("tydaro-storage-{}-{number}", std::process::id()));
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
                    content: serde_json::json!({"zoom": 1.5}),
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
            info: PanelInfo::Panel(
                serde_json::json!({"extension_version": 7, "future": [1, 2, 3]}),
            ),
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
}
