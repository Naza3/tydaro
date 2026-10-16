//! Cross-window docking is coordinated by the application, independently of
//! GPUI Kit's dock-local tab drag. Coordinates here are desktop *device pixels*,
//! so source and destination windows may use different scale factors.

use std::collections::BTreeMap;

use gpui_kit::{
    App, Bounds, Canvas, DisplayId, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels, Point,
    Styled, Window, canvas, point, px,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ScreenPoint {
    pub(crate) x: f32,
    pub(crate) y: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct ScreenRect {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) width: f32,
    pub(super) height: f32,
}

impl ScreenRect {
    pub(super) fn contains(self, point: ScreenPoint) -> bool {
        self.width > 0.0
            && self.height > 0.0
            && point.x >= self.x
            && point.y >= self.y
            && point.x < self.x + self.width
            && point.y < self.y + self.height
    }

    pub(super) fn preview(self, side: DockSide) -> Self {
        match side {
            DockSide::Center => self,
            DockSide::Left => Self {
                width: self.width / 2.0,
                ..self
            },
            DockSide::Right => Self {
                x: self.x + self.width / 2.0,
                width: self.width / 2.0,
                ..self
            },
            DockSide::Top => Self {
                height: self.height / 2.0,
                ..self
            },
            DockSide::Bottom => Self {
                y: self.y + self.height / 2.0,
                height: self.height / 2.0,
                ..self
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DockSide {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

/// The target is a visible business panel. `None` denotes an empty window's
/// workspace, where the caller should create the first dock group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DropTarget {
    pub(super) window: u64,
    pub(super) panel: Option<u64>,
    pub(super) side: DockSide,
    pub(super) bounds: ScreenRect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum TransferIntent {
    Dock {
        source_window: u64,
        panel: u64,
        target: DropTarget,
    },
    Detach {
        source_window: u64,
        panel: u64,
        position: ScreenPoint,
    },
}

#[derive(Clone, Copy, Debug)]
struct DragSession {
    source_window: u64,
    panel: u64,
    origin: ScreenPoint,
    moved: bool,
}

#[derive(Clone, Copy, Debug)]
struct PanelSurface {
    window: u64,
    bounds: ScreenRect,
}

#[derive(Default)]
pub(super) struct DragCoordinator {
    windows: BTreeMap<u64, ScreenRect>,
    panels: BTreeMap<u64, PanelSurface>,
    /// Counts durable panels including inactive tabs, not only painted surfaces.
    window_panel_counts: BTreeMap<u64, usize>,
    /// Oldest/backmost first. Update when GPUI reports a window activation.
    stacking: Vec<u64>,
    session: Option<DragSession>,
    pub(super) target: Option<DropTarget>,
}

impl DragCoordinator {
    /// Call with current workspace bounds during prepaint. Existing windows
    /// retain their stacking position until `raise_window` is called.
    pub(super) fn register_window(&mut self, window: u64, bounds: ScreenRect) {
        if self.windows.insert(window, bounds).is_none() {
            self.stacking.push(window);
        }
    }

    /// Begin a window's prepaint registration pass before its visible panel
    /// bodies register themselves, to discard bounds of inactive/closed tabs.
    pub(super) fn clear_window_panels(&mut self, window: u64) {
        self.panels.retain(|_, surface| surface.window != window);
    }

    pub(super) fn register_panel(&mut self, window: u64, panel: u64, bounds: ScreenRect) {
        self.panels.insert(panel, PanelSurface { window, bounds });
    }

    pub(super) fn set_window_panel_count(&mut self, window: u64, count: usize) {
        self.window_panel_counts.insert(window, count);
    }

    pub(super) fn raise_window(&mut self, window: u64) {
        if self.windows.contains_key(&window) {
            self.stacking.retain(|id| *id != window);
            self.stacking.push(window);
        }
    }

    pub(super) fn remove_window(&mut self, window: u64) {
        self.windows.remove(&window);
        self.window_panel_counts.remove(&window);
        self.clear_window_panels(window);
        self.stacking.retain(|id| *id != window);
        if self.session.is_some_and(|s| s.source_window == window) {
            self.cancel();
        } else if self.target.is_some_and(|target| target.window == window) {
            self.target = None;
        }
    }

    /// Start on the dedicated transfer grip's left mouse-down. GPUI's Windows
    /// backend calls native SetCapture on mouse-down and releases on mouse-up;
    /// X11 supplies an implicit grab. Neither requires another application to
    /// receive our drag payload.
    pub(super) fn begin(&mut self, source_window: u64, panel: u64, origin: ScreenPoint) {
        self.session = Some(DragSession {
            source_window,
            panel,
            origin,
            moved: false,
        });
        self.target = None;
        self.raise_window(source_window);
    }

    pub(super) fn is_active(&self) -> bool {
        self.session.is_some()
    }

    pub(super) fn update(&mut self, position: ScreenPoint) -> Option<DropTarget> {
        let session = self.session.as_mut()?;
        let dx = position.x - session.origin.x;
        let dy = position.y - session.origin.y;
        session.moved |= dx * dx + dy * dy >= 36.0;
        self.target = if session.moved {
            self.hit_test(position)
        } else {
            None
        };
        self.target
    }

    pub(super) fn finish(&mut self, position: ScreenPoint) -> Option<TransferIntent> {
        self.update(position);
        let session = self.session.take()?;
        let target = self.target.take();
        if !session.moved {
            return None;
        }
        match target {
            Some(target) if target.panel == Some(session.panel) => None,
            Some(target) => Some(TransferIntent::Dock {
                source_window: session.source_window,
                panel: session.panel,
                target,
            }),
            None => Some(TransferIntent::Detach {
                source_window: session.source_window,
                panel: session.panel,
                position,
            }),
        }
    }

    /// Invoke on Escape, window destruction or loss of the source drag button.
    pub(super) fn cancel(&mut self) {
        self.session = None;
        self.target = None;
    }

    fn hit_test(&self, position: ScreenPoint) -> Option<DropTarget> {
        let contains = |id: &&u64| {
            self.windows
                .get(id)
                .is_some_and(|bounds| bounds.contains(position))
        };
        let front = self.stacking.iter().rev().find(contains)?;
        // A newly opened single-component window usually sits above its intended
        // destination. During its component drag, treat just this source as a
        // transparent drag surface so it cannot mask the main workspace. Keep a
        // self target when nothing lies behind it (a self-drop remains a no-op).
        // Multi-panel sources are never skipped: their other groups remain valid
        // destinations, and inactive tabs count towards this decision.
        let source = self.session.map(|session| session.source_window);
        let window = if source == Some(*front)
            && *front != 0
            && self.window_panel_counts.get(front) == Some(&1)
        {
            self.stacking
                .iter()
                .rev()
                .filter(|id| **id != *front)
                .find(contains)
                .unwrap_or(front)
        } else {
            front
        };
        let bounds = *self.windows.get(window)?;
        if let Some((panel, surface)) = self
            .panels
            .iter()
            .find(|(_, surface)| surface.window == *window && surface.bounds.contains(position))
        {
            Some(DropTarget {
                window: *window,
                panel: Some(*panel),
                side: dock_side(surface.bounds, position),
                bounds: surface.bounds,
            })
        } else {
            // The window may be empty or the pointer may be in its tab strip.
            // Let the caller append to the window's first/empty dock group.
            Some(DropTarget {
                window: *window,
                panel: None,
                side: DockSide::Center,
                bounds,
            })
        }
    }
}

/// The nearest edge occupies the outer quarter; the center merges as a tab.
pub(super) fn dock_side(bounds: ScreenRect, position: ScreenPoint) -> DockSide {
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return DockSide::Center;
    }
    let x = (position.x - bounds.x) / bounds.width;
    let y = (position.y - bounds.y) / bounds.height;
    [
        (x, DockSide::Left),
        (1.0 - x, DockSide::Right),
        (y, DockSide::Top),
        (1.0 - y, DockSide::Bottom),
    ]
    .into_iter()
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .filter(|(distance, _)| *distance < 0.25)
    .map_or(DockSide::Center, |(_, side)| side)
}

/// GPUI Windows and X11 store a window's desktop origin in logical pixels
/// divided by that window's scale. Convert both origin and local coordinates
/// to device pixels before comparing two windows. Wayland does not expose
/// desktop positions; cross-window dragging there requires a future native
/// data-device implementation, and callers should hide the transfer grip.
pub(super) fn screen_point(window: &Window, local: Point<Pixels>) -> ScreenPoint {
    let origin = window.bounds().origin;
    let scale = window.scale_factor();
    ScreenPoint {
        x: (origin.x.as_f32() + local.x.as_f32()) * scale,
        y: (origin.y.as_f32() + local.y.as_f32()) * scale,
    }
}

pub(super) fn screen_rect(window: &Window, local: Bounds<Pixels>) -> ScreenRect {
    let origin = screen_point(window, local.origin);
    ScreenRect {
        x: origin.x,
        y: origin.y,
        width: local.size.width.as_f32() * window.scale_factor(),
        height: local.size.height.as_f32() * window.scale_factor(),
    }
}

/// Convert a desktop drop point into the destination monitor's logical space.
/// GPUI's public display interface omits DPI, so Windows needs this narrow
/// native query when no existing application window occupies that monitor.
/// Merely dividing by the source window's scale is wrong on mixed-DPI setups.
pub(super) fn detached_window_origin(
    position: ScreenPoint,
    source_scale: f32,
    cx: &App,
) -> (Point<Pixels>, Option<DisplayId>) {
    #[cfg(target_os = "windows")]
    if let Some((monitor, scale)) = windows_monitor_at(position) {
        let display = cx
            .displays()
            .iter()
            .find(|d| u64::from(d.id()) == monitor)
            .map(|d| d.id());
        return (
            point(px(position.x / scale), px(position.y / scale)),
            display,
        );
    }
    // X11 uses a common desktop scale. Wayland doesn't offer desktop positions
    // and the application hides its desktop-transfer grip on that backend.
    let scale = source_scale.max(0.01);
    let origin = point(px(position.x / scale), px(position.y / scale));
    let display = cx
        .displays()
        .iter()
        .find(|d| d.bounds().contains(&origin))
        .map(|d| d.id());
    (origin, display)
}

#[cfg(target_os = "windows")]
fn windows_monitor_at(position: ScreenPoint) -> Option<(u64, f32)> {
    #[repr(C)]
    struct NativePoint {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        #[link_name = "MonitorFromPoint"]
        fn monitor_from_point(point: NativePoint, flags: u32) -> isize;
    }
    #[link(name = "shcore")]
    unsafe extern "system" {
        #[link_name = "GetDpiForMonitor"]
        fn get_dpi_for_monitor(
            monitor: isize,
            dpi_type: i32,
            dpi_x: *mut u32,
            dpi_y: *mut u32,
        ) -> i32;
    }
    // Both calls are read-only Win32 queries. NativePoint has POINT's C layout,
    // MONITOR_DEFAULTTONEAREST is 2, MDT_EFFECTIVE_DPI is 0, and the output
    // pointers refer to live local u32 values for the duration of the call.
    let monitor = unsafe {
        monitor_from_point(
            NativePoint {
                x: position.x.round() as i32,
                y: position.y.round() as i32,
            },
            2,
        )
    };
    if monitor == 0 {
        return None;
    }
    let mut dpi_x = 0;
    let mut dpi_y = 0;
    let result = unsafe { get_dpi_for_monitor(monitor, 0, &mut dpi_x, &mut dpi_y) };
    (result >= 0 && dpi_x > 0).then_some((monitor as u64, dpi_x as f32 / 96.0))
}

/// Put this zero-size canvas in each window's root. Unlike `Div::on_mouse_move`,
/// these paint-phase window listeners are not limited by a local hitbox, so a
/// captured drag continues when the pointer leaves the source native window.
/// The callbacks decide whether a transfer is active and stop propagation only
/// for that transfer, leaving regular dock tab drags untouched.
pub(super) fn window_event_listener(
    mut on_move: impl FnMut(&MouseMoveEvent, &mut Window, &mut App) + 'static,
    mut on_release: impl FnMut(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> Canvas<()> {
    canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                if phase.capture() {
                    on_move(event, window, cx);
                }
            });
            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                if phase.capture() && event.button == MouseButton::Left {
                    on_release(event, window, cx);
                }
            });
        },
    )
    .w(px(0.0))
    .h(px(0.0))
}

#[cfg(test)]
mod tests;

#[cfg(feature = "test-support")]
mod test_support;
