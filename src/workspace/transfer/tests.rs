//! Unit regressions for this module.

use super::*;

fn point(x: f32, y: f32) -> ScreenPoint {
    ScreenPoint { x, y }
}
fn rect(x: f32, y: f32, width: f32, height: f32) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn edges_and_center_are_scale_independent() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let bounds = rect(-1200.0, 80.0, 800.0 * scale, 400.0 * scale);
        for (x, y, side) in [
            (0.1, 0.5, DockSide::Left),
            (0.9, 0.5, DockSide::Right),
            (0.5, 0.1, DockSide::Top),
            (0.5, 0.9, DockSide::Bottom),
            (0.5, 0.5, DockSide::Center),
        ] {
            assert_eq!(
                dock_side(
                    bounds,
                    point(bounds.x + bounds.width * x, bounds.y + bounds.height * y)
                ),
                side
            );
        }
    }
}

#[test]
fn cross_window_move_preserves_identity_and_selects_destination_edge() {
    let mut drag = DragCoordinator::default();
    drag.register_window(1, rect(0.0, 0.0, 800.0, 600.0));
    drag.register_panel(1, 10, rect(0.0, 0.0, 800.0, 600.0));
    drag.register_window(2, rect(1000.0, 0.0, 1200.0, 900.0));
    drag.register_panel(2, 20, rect(1000.0, 0.0, 1200.0, 900.0));
    drag.begin(1, 10, point(50.0, 30.0));
    let intent = drag.finish(point(1100.0, 450.0)).unwrap();
    let TransferIntent::Dock {
        source_window,
        panel,
        target,
    } = intent
    else {
        panic!("expected a dock transfer");
    };
    assert_eq!(
        (source_window, panel, target.window, target.panel),
        (1, 10, 2, Some(20))
    );
    assert_eq!(target.side, DockSide::Left);
    assert!(!drag.is_active());
}

#[test]
fn click_and_self_drop_do_not_detach() {
    let mut drag = DragCoordinator::default();
    drag.register_window(1, rect(0.0, 0.0, 800.0, 600.0));
    drag.register_panel(1, 10, rect(0.0, 0.0, 800.0, 600.0));
    drag.begin(1, 10, point(10.0, 10.0));
    assert_eq!(drag.finish(point(12.0, 12.0)), None);
    drag.begin(1, 10, point(10.0, 10.0));
    assert_eq!(drag.finish(point(600.0, 300.0)), None);
}

#[test]
fn desktop_drop_detaches_and_closed_window_cannot_receive() {
    let mut drag = DragCoordinator::default();
    drag.register_window(1, rect(0.0, 0.0, 800.0, 600.0));
    drag.register_window(2, rect(1000.0, 0.0, 800.0, 600.0));
    drag.register_panel(2, 20, rect(1000.0, 0.0, 800.0, 600.0));
    drag.remove_window(2);
    drag.begin(1, 10, point(10.0, 10.0));
    assert_eq!(
        drag.finish(point(1200.0, 300.0)),
        Some(TransferIntent::Detach {
            source_window: 1,
            panel: 10,
            position: point(1200.0, 300.0),
        })
    );
}

#[test]
fn top_window_blocks_panels_behind_it() {
    let mut drag = DragCoordinator::default();
    drag.register_window(1, rect(0.0, 0.0, 800.0, 600.0));
    drag.register_window(2, rect(0.0, 0.0, 800.0, 600.0));
    drag.register_panel(1, 10, rect(0.0, 0.0, 800.0, 600.0));
    drag.register_panel(2, 20, rect(0.0, 0.0, 800.0, 600.0));
    drag.begin(1, 10, point(10.0, 10.0));
    drag.raise_window(2);
    assert_eq!(drag.update(point(400.0, 300.0)).unwrap().panel, Some(20));
    drag.clear_window_panels(2);
    assert_eq!(drag.update(point(400.0, 300.0)).unwrap().panel, None);
}

#[test]
fn singleton_source_does_not_occlude_underlying_docking_target() {
    let mut drag = DragCoordinator::default();
    drag.register_window(0, rect(0., 0., 1200., 800.));
    drag.register_panel(0, 10, rect(0., 0., 1200., 800.));
    drag.register_window(1, rect(100., 100., 320., 500.));
    drag.register_panel(1, 20, rect(100., 100., 320., 500.));
    drag.set_window_panel_count(1, 1);
    drag.begin(1, 20, point(150., 110.));
    let target = drag.update(point(120., 300.)).unwrap();
    assert_eq!(
        (target.window, target.panel, target.side),
        (0, Some(10), DockSide::Left)
    );
    assert!(matches!(
        drag.finish(point(120., 300.)),
        Some(TransferIntent::Dock { panel: 20, .. })
    ));
}

#[test]
fn inactive_tabs_keep_multi_panel_source_as_a_local_target() {
    let mut drag = DragCoordinator::default();
    drag.register_window(0, rect(0., 0., 1200., 800.));
    drag.register_panel(0, 10, rect(0., 0., 1200., 800.));
    drag.register_window(1, rect(100., 100., 600., 500.));
    // Only one painted tab, but another durable panel is inactive.
    drag.register_panel(1, 20, rect(100., 100., 600., 500.));
    drag.set_window_panel_count(1, 2);
    drag.begin(1, 20, point(150., 110.));
    assert_eq!(drag.finish(point(120., 300.)), None);
    // Another visible group in that same source must remain dockable too.
    drag.register_panel(1, 21, rect(400., 100., 300., 500.));
    drag.register_panel(1, 20, rect(100., 100., 300., 500.));
    drag.begin(1, 20, point(150., 110.));
    let target = drag.update(point(500., 150.)).unwrap();
    assert_eq!(
        (target.window, target.panel, target.side),
        (1, Some(21), DockSide::Top)
    );
}

#[test]
fn singleton_self_drop_without_underlying_window_is_still_a_noop() {
    let mut drag = DragCoordinator::default();
    drag.register_window(1, rect(100., 100., 320., 500.));
    drag.register_panel(1, 20, rect(100., 100., 320., 500.));
    drag.set_window_panel_count(1, 1);
    drag.begin(1, 20, point(150., 110.));
    assert_eq!(drag.finish(point(120., 300.)), None);
}

#[test]
fn cancellation_clears_preview_and_prevents_any_transfer() {
    let mut drag = DragCoordinator::default();
    drag.register_window(0, rect(0., 0., 1200., 800.));
    drag.register_window(1, rect(1400., 0., 320., 500.));
    drag.begin(1, 20, point(1450., 30.));
    assert!(drag.update(point(100., 100.)).is_some());
    drag.cancel();
    assert!(!drag.is_active());
    assert!(drag.target.is_none());
    assert_eq!(drag.finish(point(100., 100.)), None);
}
