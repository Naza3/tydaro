//! Pure workspace snapshots and validation. No filesystem or live view state.
use crate::model::PanelModel;
use anyhow::{Context, Result, bail, ensure};
use gpui_kit::component::dock::{DockAreaState, PanelInfo, PanelState};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_STABLE_ID: u64 = u64::MAX / 2;

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
        crate::panel_snapshot::validate(&saved.content)
            .with_context(|| format!("Invalid content for panel {}", saved.model.id))?;
        ensure!(saved.model.id <= MAX_STABLE_ID, "Invalid panel ID");
        ensure!(saved.model.link_group <= 3, "Invalid panel link group");
        if let Some(name) = &saved.model.custom_name {
            ensure!(
                PanelModel::normalize_name(name)?.as_deref() == Some(name.as_str()),
                "Invalid custom panel name"
            );
        }
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
