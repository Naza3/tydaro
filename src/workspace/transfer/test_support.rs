//! Geometry invariants exercised by native workspace tests.
use super::{DragCoordinator, ScreenPoint, ScreenRect};

impl DragCoordinator {
    pub(in crate::workspace) fn verify_window_geometry(
        &self,
        window: u64,
        expected: ScreenRect,
    ) -> anyhow::Result<()> {
        let actual = self
            .windows
            .get(&window)
            .ok_or_else(|| anyhow::anyhow!("No docking geometry registered for window {window}"))?;
        anyhow::ensure!(
            (actual.x - expected.x).abs() < 1.0
                && (actual.y - expected.y).abs() < 1.0
                && (actual.width - expected.width).abs() < 1.0
                && (actual.height - expected.height).abs() < 1.0,
            "Window {window} docking bounds differ from its visible dock: {actual:?} vs {expected:?}"
        );
        let panels: Vec<_> = self
            .panels
            .iter()
            .filter(|(_, p)| p.window == window)
            .collect();
        anyhow::ensure!(
            !panels.is_empty(),
            "Window {window} has no registered visible panels"
        );
        for (id, panel) in panels {
            let center = ScreenPoint {
                x: panel.bounds.x + panel.bounds.width / 2.0,
                y: panel.bounds.y + panel.bounds.height / 2.0,
            };
            anyhow::ensure!(
                expected.contains(center),
                "Panel {id} drag geometry lies outside its visible workspace: {:?}",
                panel.bounds
            );
        }
        Ok(())
    }
}
