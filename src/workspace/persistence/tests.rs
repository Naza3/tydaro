//! Layout history and snapshot compatibility regressions.
use crate::workspace::snapshot::SavedWindow;

use super::{geometry_changed, layout_key, preserve_live_state, saved_panel_ids};
use crate::market::{Period, Symbol};
use crate::model::{PanelKind, PanelModel};
use crate::workspace::layout::{BoardPreset, preset_workspace, remap_workspace_ids};
use crate::workspace::snapshot::{SCHEMA_VERSION, SavedPanel, WorkspaceFile};
use gpui_kit::{
    component::dock::{DockAreaState, PanelInfo, PanelState},
    px,
};

fn fixture() -> WorkspaceFile {
    let mut content = crate::panels::snapshot::ContentSnapshot::new("100.00".into());
    content.query = "AAPL".into();
    content.chart.bars_visible = 80;
    let panel = SavedPanel {
        model: PanelModel::new(7, PanelKind::Chart),
        content: serde_json::to_value(content).unwrap(),
    };
    WorkspaceFile {
        version: SCHEMA_VERSION,
        next_id: 8,
        paused: false,
        windows: vec![SavedWindow {
            id: 0,
            bounds: [10., 10., 1400., 900.],
            locked: false,
            dock: DockAreaState {
                center: PanelState {
                    panel_name: "StackPanel".into(),
                    info: PanelInfo::Stack {
                        sizes: vec![px(1400.)],
                        axis: 0,
                    },
                    children: vec![PanelState {
                        panel_name: "tydaro.panel".into(),
                        children: vec![],
                        info: PanelInfo::Panel(serde_json::to_value(panel).unwrap()),
                    }],
                },
                ..Default::default()
            },
        }],
    }
}

fn payload(state: &WorkspaceFile) -> SavedPanel {
    let PanelInfo::Panel(value) = &state.windows[0].dock.center.children[0].info else {
        panic!("expected panel");
    };
    serde_json::from_value(value.clone()).unwrap()
}

#[test]
fn board_presets_have_valid_dock_trees_and_bounded_fresh_identities() {
    for (preset, count) in [
        (BoardPreset::Empty, 0),
        (BoardPreset::Standard, 9),
        (BoardPreset::DualChart, 10),
    ] {
        let state = preset_workspace(preset, 100, [40., 40., 1536., 930.], true).unwrap();
        crate::workspace::snapshot::validate(&state).unwrap();
        assert_eq!(state.windows.len(), 1);
        assert_eq!(state.windows[0].id, 0);
        assert!(state.paused);
        assert!(!state.windows[0].dock.center.panel_name.is_empty());
        let ids = saved_panel_ids(&state);
        assert_eq!(ids.len(), count);
        assert!(ids.iter().all(|id| *id >= 100));
        assert_eq!(state.next_id, 100 + count as u64);
    }
    assert!(
        preset_workspace(
            BoardPreset::Standard,
            crate::workspace::snapshot::MAX_STABLE_ID - 1,
            [40., 40., 1536., 930.],
            false,
        )
        .is_err()
    );
}

#[test]
fn board_copy_remaps_floats_hidden_tabs_and_side_docks_without_changing_content() {
    use gpui_kit::component::dock::{DockPlacement, DockState};
    let mut state = fixture();
    let original_panel = state.windows[0].dock.center.children[0].clone();
    let original_content = payload(&state).content;
    let mut floating = state.windows[0].clone();
    floating.id = 20;
    let mut side_panel = original_panel.clone();
    let PanelInfo::Panel(value) = &mut side_panel.info else {
        unreachable!()
    };
    value["model"]["id"] = serde_json::json!(21);
    floating.dock.center = PanelState {
        panel_name: "TabPanel".into(),
        children: vec![],
        info: PanelInfo::Tabs { active_index: 0 },
    };
    floating.dock.left_dock = Some(DockState::new(
        PanelState {
            panel_name: "future.extension".into(),
            children: vec![side_panel],
            info: PanelInfo::Panel(serde_json::json!({"unknown": [1, "keep"]})),
        },
        DockPlacement::Left,
        px(210.),
        false,
    ));
    state.windows.push(floating);
    let before = state.clone();
    remap_workspace_ids(&mut state, 100).unwrap();
    assert_eq!(saved_panel_ids(&state), [100, 102].into_iter().collect());
    assert_eq!(state.windows[0].id, 0);
    assert_eq!(state.windows[1].id, 101);
    assert_eq!(state.next_id, 103);
    assert_eq!(payload(&state).content, original_content);
    assert_eq!(state.windows[1].bounds, before.windows[1].bounds);
    let side = state.windows[1].dock.left_dock.as_ref().unwrap();
    assert!(!side.open());
    assert_eq!(side.size(), px(210.));
    assert_eq!(
        side.panel().info,
        before.windows[1]
            .dock
            .left_dock
            .as_ref()
            .unwrap()
            .panel()
            .info
    );
    assert!(saved_panel_ids(&before).is_disjoint(&saved_panel_ids(&state)));
}

#[test]
fn history_ignores_business_state_but_detects_layout_and_lock_changes() {
    let original = fixture();
    let mut changed = original.clone();
    changed.next_id = 100;
    changed.paused = true;
    changed.windows[0].bounds = [100., 60., 1600., 1000.];
    let mut saved = payload(&changed);
    saved.model.symbol = Symbol::Msft;
    saved.model.period = Period::Week;
    saved.model.custom_name = Some("趋势组".into());
    saved.content["query"] = serde_json::json!("MSFT");
    saved.content["chart"]["bars_visible"] = serde_json::json!(40);
    changed.windows[0].dock.center.children[0].info =
        PanelInfo::Panel(serde_json::to_value(saved).unwrap());
    assert_eq!(layout_key(&original, false), layout_key(&changed, false));
    assert!(geometry_changed(&original, &changed));
    changed.windows[0].dock.center.info = PanelInfo::Stack {
        sizes: vec![px(1600.)],
        axis: 0,
    };
    assert_ne!(layout_key(&original, false), layout_key(&changed, false));
    assert_eq!(layout_key(&original, true), layout_key(&changed, true));
    changed.windows[0].locked = true;
    assert_ne!(layout_key(&original, true), layout_key(&changed, true));
}

#[test]
fn undo_preserves_live_form_viewport_symbol_and_pause_state() {
    let mut historical = fixture();
    let mut live = fixture();
    let mut saved = payload(&live);
    saved.model.symbol = Symbol::Nvda;
    saved.model.period = Period::Day;
    saved.model.link_group = 3;
    saved.model.custom_name = Some("科技趋势".into());
    saved.content["quantity"] = serde_json::json!("500");
    saved.content["chart"]["bars_visible"] = serde_json::json!(120);
    live.windows[0].dock.center.children[0].info =
        PanelInfo::Panel(serde_json::to_value(&saved).unwrap());
    live.paused = true;
    live.next_id = 20;
    live.windows[0].bounds = [300., 80., 1600., 1000.];
    preserve_live_state(&mut historical, &live);
    let restored = payload(&historical);
    assert_eq!(restored.model.symbol, Symbol::Nvda);
    assert_eq!(restored.model.period, Period::Day);
    assert_eq!(restored.model.link_group, 1);
    assert_eq!(restored.model.custom_name, saved.model.custom_name);
    assert_eq!(restored.content, saved.content);
    assert!(historical.paused);
    assert_eq!(historical.next_id, 20);
    assert_eq!(historical.windows[0].bounds, live.windows[0].bounds);
}
