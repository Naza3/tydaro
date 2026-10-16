//! Built-in layouts and identity-safe copying of saved dock trees.
use super::{
    StockPanel, allocate,
    snapshot::{SavedPanel, SavedWindow, WorkspaceFile},
};
use crate::model::{PanelKind, PanelModel};
use gpui_kit::{component::dock::*, *};

#[derive(Clone, Copy)]
pub(crate) enum BoardPreset {
    Empty,
    Standard,
    DualChart,
}

pub(super) fn default_layout(
    dock: &Entity<DockArea>,
    owner: u64,
    dual: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let mut one = |kind, cx: &mut App| {
        let id = allocate(cx);
        let p = StockPanel::create(
            owner,
            SavedPanel {
                model: PanelModel::new(id, kind),
                content: serde_json::Value::Null,
            },
            window,
            cx,
        )
        .expect("empty content creates a default panel");
        DockLayout::tabs().panel_view(panel_handle(p), cx)
    };
    let watch = one(PanelKind::Watchlist, cx);
    let chart = one(PanelKind::Chart, cx);
    let chart = if dual {
        DockLayout::h_split()
            .child(chart, None)
            .child(one(PanelKind::Chart, cx), None)
    } else {
        chart
    };
    let quote = one(PanelKind::Quote, cx);
    let book = one(PanelKind::OrderBook, cx);
    let trades = one(PanelKind::Trades, cx);
    let mut bottom = DockLayout::tabs();
    for kind in [
        PanelKind::OrderEntry,
        PanelKind::Orders,
        PanelKind::Positions,
        PanelKind::News,
    ] {
        let id = allocate(cx);
        let p = StockPanel::create(
            owner,
            SavedPanel {
                model: PanelModel::new(id, kind),
                content: serde_json::Value::Null,
            },
            window,
            cx,
        )
        .expect("empty content creates a default panel");
        bottom = bottom.panel_view(panel_handle(p), cx);
    }
    let layout = DockLayout::h_split()
        .child(watch, Some(px(232.)))
        .child(
            DockLayout::v_split()
                .child(chart, None)
                .child(bottom, Some(px(260.))),
            None,
        )
        .child(
            DockLayout::v_split()
                .child(quote, Some(px(320.)))
                .child(book, Some(px(300.)))
                .child(trades, None),
            Some(px(320.)),
        );
    dock.update(cx, |d, cx| d.set_center(layout, window, cx));
}
pub(super) fn empty_tabs() -> PanelState {
    PanelState {
        panel_name: "TabPanel".into(),
        children: vec![],
        info: PanelInfo::Tabs { active_index: 0 },
    }
}

fn next_saved_id(next_id: &mut u64) -> anyhow::Result<u64> {
    anyhow::ensure!(
        *next_id < crate::workspace::snapshot::MAX_STABLE_ID,
        "组件编号已用尽"
    );
    let id = *next_id;
    *next_id += 1;
    Ok(id)
}

pub(super) fn preset_workspace(
    preset: BoardPreset,
    mut next_id: u64,
    bounds: [f32; 4],
    paused: bool,
) -> anyhow::Result<WorkspaceFile> {
    fn tabs(kinds: &[PanelKind], next_id: &mut u64) -> anyhow::Result<PanelState> {
        let mut tab = empty_tabs();
        for kind in kinds {
            let saved = SavedPanel {
                model: PanelModel::new(next_saved_id(next_id)?, *kind),
                content: serde_json::Value::Null,
            };
            tab.children.push(PanelState {
                panel_name: "tydaro.panel".into(),
                children: vec![],
                info: PanelInfo::Panel(serde_json::to_value(saved)?),
            });
        }
        Ok(tab)
    }
    fn stack(axis: usize, slots: Vec<(PanelState, f32)>) -> PanelState {
        let (children, sizes): (Vec<_>, Vec<_>) =
            slots.into_iter().map(|(p, s)| (p, px(s))).unzip();
        PanelState {
            panel_name: "StackPanel".into(),
            children,
            info: PanelInfo::Stack { axis, sizes },
        }
    }
    let center = match preset {
        BoardPreset::Empty => empty_tabs(),
        BoardPreset::Standard | BoardPreset::DualChart => {
            let main_width = (bounds[2] - 120. - 232. - 320.).max(320.);
            let height = (bounds[3] - 73.).max(500.);
            let watch = tabs(&[PanelKind::Watchlist], &mut next_id)?;
            let chart = tabs(&[PanelKind::Chart], &mut next_id)?;
            let chart = if matches!(preset, BoardPreset::DualChart) {
                stack(
                    0,
                    vec![
                        (chart, main_width / 2.),
                        (tabs(&[PanelKind::Chart], &mut next_id)?, main_width / 2.),
                    ],
                )
            } else {
                chart
            };
            let bottom = tabs(
                &[
                    PanelKind::OrderEntry,
                    PanelKind::Orders,
                    PanelKind::Positions,
                    PanelKind::News,
                ],
                &mut next_id,
            )?;
            let quote = tabs(&[PanelKind::Quote], &mut next_id)?;
            let book = tabs(&[PanelKind::OrderBook], &mut next_id)?;
            let trades = tabs(&[PanelKind::Trades], &mut next_id)?;
            stack(
                0,
                vec![
                    (watch, 232.),
                    (
                        stack(1, vec![(chart, height - 260.), (bottom, 260.)]),
                        main_width,
                    ),
                    (
                        stack(
                            1,
                            vec![
                                (quote, 320.),
                                (book, 300.),
                                (trades, (height - 620.).max(100.)),
                            ],
                        ),
                        320.,
                    ),
                ],
            )
        }
    };
    let state = WorkspaceFile {
        version: crate::workspace::snapshot::SCHEMA_VERSION,
        next_id,
        paused,
        windows: vec![SavedWindow {
            id: 0,
            bounds,
            locked: false,
            dock: DockAreaState {
                version: Some(1),
                center,
                ..Default::default()
            },
        }],
    };
    crate::workspace::snapshot::validate(&state)?;
    Ok(state)
}

/// Copy identities as well as window-local entities. Hidden tabs and side docks
/// participate; payloads of unknown panels stay byte-for-byte equivalent JSON.
pub(super) fn remap_workspace_ids(
    state: &mut WorkspaceFile,
    mut next_id: u64,
) -> anyhow::Result<()> {
    fn remap(panel: &mut PanelState, next_id: &mut u64) -> anyhow::Result<()> {
        if panel.panel_name == "tydaro.panel"
            && let PanelInfo::Panel(value) = &mut panel.info
        {
            let mut saved: SavedPanel = serde_json::from_value(value.clone())?;
            saved.model.id = next_saved_id(next_id)?;
            *value = serde_json::to_value(saved)?;
        }
        for child in &mut panel.children {
            remap(child, next_id)?;
        }
        Ok(())
    }
    for window in &mut state.windows {
        if window.id != 0 {
            window.id = next_saved_id(&mut next_id)?;
        }
        remap(&mut window.dock.center, &mut next_id)?;
        for dock in [
            &mut window.dock.left_dock,
            &mut window.dock.right_dock,
            &mut window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            let mut panel = dock.panel().clone();
            remap(&mut panel, &mut next_id)?;
            *dock = DockState::new(panel, dock.placement(), dock.size(), dock.open());
        }
    }
    state.next_id = next_id;
    crate::workspace::snapshot::validate(state)
}
