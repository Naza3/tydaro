//! Native window construction, placement and retirement.
use super::{Session, changed, default_layout, record, report, save_now, status, workspace};
use super::{
    snapshot::SavedWindow,
    transfer::{self, ScreenPoint},
};
use crate::model::PanelKind;
use gpui_kit::{component::dock::*, prelude::*, *};

pub(crate) struct Workspace {
    pub(super) id: u64,
    pub(crate) handle: AnyWindowHandle,
    pub(crate) dock: Entity<DockArea>,
    pub(super) bounds: [f32; 4],
    pub(super) main_content: Option<AnyView>,
    _subscriptions: Vec<Subscription>,
}
impl Workspace {
    fn new(
        id: u64,
        saved: Option<SavedWindow>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (dock, skin) = DockSkin::dock_area(format!("workspace-{id}"), Some(1), window, cx);
        skin.set_close_button_visible(true, cx);
        cx.global_mut::<Session>().building_owner = id;
        let loaded = saved.map(|saved| {
            dock.update(cx, |d, cx| {
                let result = d.load(saved.dock, window, cx);
                d.set_locked(saved.locked, window, cx);
                result
            })
        });
        if let Some(Err(ref err)) = loaded {
            status(format!("布局恢复失败：{err}"), cx);
        }
        if loaded.is_none() && id == 0 {
            default_layout(&dock, id, false, window, cx);
        }
        let sub = cx.subscribe(&dock, |_, _, _: &DockEvent, cx| cx.defer(changed));
        let bounds_sub = cx.observe_window_bounds(window, |this, window, cx| {
            this.bounds = window_bounds(window);
            changed(cx);
        });
        let active = cx.observe_window_activation(window, move |_, window, cx| {
            if window.is_window_active() {
                cx.global_mut::<Session>().drag.raise_window(id);
            }
        });
        let entity_id = cx.entity_id();
        window.on_window_should_close(cx, move |window, cx| {
            if id == 0 {
                (cx.global::<Session>().host.close_main)(window, cx)
            } else {
                cx.defer(move |cx| {
                    // A queued native request must not dismiss a replacement
                    // window created by a board switch or layout restoration.
                    if workspace(id, cx).is_some_and(|view| view.entity_id() == entity_id) {
                        let r = dismiss_window(id, cx);
                        report(r, cx);
                    }
                });
                false
            }
        });
        let main_content =
            (id == 0).then(|| (cx.global::<Session>().host.build_main)(dock.clone(), window, cx));
        Self {
            id,
            handle: window.window_handle(),
            dock,
            bounds: window_bounds(window),
            main_content,
            _subscriptions: vec![sub, bounds_sub, active],
        }
    }
}
pub(super) fn window_bounds(window: &Window) -> [f32; 4] {
    let b = window.bounds();
    [
        b.origin.x.into(),
        b.origin.y.into(),
        b.size.width.into(),
        b.size.height.into(),
    ]
}
pub(crate) fn open_workspace(
    id: u64,
    saved: Option<SavedWindow>,
    cx: &mut App,
) -> anyhow::Result<Entity<Workspace>> {
    open_workspace_on_display(id, saved, None, None, cx)
}
pub(super) fn open_workspace_on_display(
    id: u64,
    saved: Option<SavedWindow>,
    display_id: Option<DisplayId>,
    bounds_override: Option<[f32; 4]>,
    cx: &mut App,
) -> anyhow::Result<Entity<Workspace>> {
    anyhow::ensure!(
        cx.global::<Session>().loading
            || workspace(id, cx).is_some()
            || cx.global::<Session>().windows.len() < 16,
        "最多支持 16 个工作区窗口"
    );
    let b = bounds_override
        .or_else(|| saved.as_ref().map(|s| s.bounds))
        .unwrap_or(if id == 0 {
            [40., 40., 1536., 930.]
        } else {
            [150., 100., 760., 570.]
        });
    let mut bounds = visible_bounds(b, id, cx);
    if let Some(display) =
        display_id.and_then(|id| cx.displays().into_iter().find(|d| d.id() == id))
    {
        let visible = display.visible_bounds();
        bounds.size.width = bounds.size.width.min(visible.size.width);
        bounds.size.height = bounds.size.height.min(visible.size.height);
        bounds.origin.x = bounds
            .origin
            .x
            .max(visible.origin.x)
            .min(visible.right() - bounds.size.width);
        bounds.origin.y = bounds
            .origin
            .y
            .max(visible.origin.y)
            .min(visible.bottom() - bounds.size.height);
    }
    let (_, view) = gpui_kit::open_window(
        WindowOptions {
            display_id,
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(if id == 0 {
                    "Tydaro · 股票工作台".into()
                } else {
                    "Tydaro · 独立组件".into()
                }),
                ..Default::default()
            }),
            window_min_size: Some(size(
                px(if id == 0 { 1024. } else { 240. }),
                px(if id == 0 { 680. } else { 240. }),
            )),
            ..Default::default()
        },
        cx,
        move |window, cx| cx.new(|cx| Workspace::new(id, saved, window, cx)),
    )?;
    let s = cx.global_mut::<Session>();
    s.windows.insert(id, view.downgrade());
    s.next_id = s.next_id.max(id + 1);
    Ok(view)
}
fn visible_bounds(b: [f32; 4], id: u64, cx: &App) -> Bounds<Pixels> {
    let mut result = Bounds::new(
        point(px(b[0]), px(b[1])),
        size(
            px(b[2].max(if id == 0 { 1024. } else { 240. })),
            px(b[3].max(if id == 0 { 680. } else { 240. })),
        ),
    );
    let displays = cx.displays();
    if let Some(display) = displays.first()
        && !displays.iter().any(|d| {
            let r = d.bounds();
            let x: f32 = r.origin.x.into();
            let y: f32 = r.origin.y.into();
            b[0] + 100. > x
                && b[0] < x + f32::from(r.size.width) - 100.
                && b[1] >= y
                && b[1] < y + f32::from(r.size.height) - 80.
        })
    {
        result.origin = display.bounds().origin + point(px(32. + id.min(8) as f32 * 20.), px(32.));
    }
    result
}
fn floating_size(kind: PanelKind) -> [f32; 2] {
    match kind {
        PanelKind::Chart => [960., 700.],
        PanelKind::Quote => [320., 390.],
        PanelKind::OrderBook => [300., 650.],
        PanelKind::Trades => [380., 580.],
        PanelKind::TradeDetails => [400., 540.],
        PanelKind::OrderDetails => [1020., 580.],
        PanelKind::Watchlist => [360., 600.],
        PanelKind::OrderEntry => [440., 580.],
        _ => [760., 530.],
    }
}

pub(super) fn floating_placement(
    owner: u64,
    kind: PanelKind,
    position: Option<ScreenPoint>,
    cx: &mut App,
) -> anyhow::Result<(Option<DisplayId>, [f32; 4])> {
    let source = workspace(owner, cx).ok_or_else(|| anyhow::anyhow!("源窗口已关闭"))?;
    let handle = source.read(cx).handle;
    let (bounds, scale, display) = handle.update(cx, |_, window, cx| {
        (
            window.bounds(),
            window.scale_factor(),
            window.display(cx).map(|d| d.id()),
        )
    })?;
    let [width, height] = floating_size(kind);
    let (origin, display) = if let Some(position) = position {
        transfer::detached_window_origin(position, scale, cx)
    } else {
        let cascade = (cx.global::<Session>().windows.len() % 6) as f32 * 24.;
        (
            bounds.origin + point(px(48. + cascade), px(48. + cascade)),
            display,
        )
    };
    Ok((
        display,
        [origin.x.as_f32(), origin.y.as_f32(), width, height],
    ))
}

pub(super) fn close_window(id: u64, cx: &mut App) {
    let handle = workspace(id, cx).map(|w| w.read(cx).handle);
    // Remove stale registry entries even if a native window already vanished.
    cx.global_mut::<Session>().windows.remove(&id);
    cx.global_mut::<Session>().drag.remove_window(id);
    if let Some(handle) = handle {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
}
pub(super) fn close_if_empty(id: u64, cx: &mut App) {
    if id != 0
        && workspace(id, cx).is_some_and(|view| {
            let dock = view.read(cx).dock.read(cx);
            // Hidden tabs, side docks and unknown panel types still own their
            // window, even when they have no visible StockPanel registration.
            [
                DockPlacement::Center,
                DockPlacement::Left,
                DockPlacement::Right,
                DockPlacement::Bottom,
            ]
            .into_iter()
            .all(|placement| {
                dock.layout(placement)
                    .is_none_or(|tree| tree.panels().next().is_none())
            })
        })
    {
        close_window(id, cx);
    }
}

/// A user closing a native floating window discards all its components.
/// Programmatic retirement (board switches, restoration and transfers) uses
/// `close_window` instead, so it cannot delete a board's saved components.
pub(crate) fn dismiss_window(id: u64, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(id != 0, "主窗口关闭时应保存并退出应用");
    if workspace(id, cx).is_none() {
        return Ok(());
    }
    // Capture current forms/viewports before any entity is released, including
    // edits made since the latest autosave. One undo restores the entire window.
    record(cx);
    let panels: Vec<_> = cx
        .global::<Session>()
        .panels
        .iter()
        .filter_map(|(panel_id, view)| {
            view.upgrade()
                .filter(|view| view.read(cx).owner == id)
                .map(|_| *panel_id)
        })
        .collect();
    let session = cx.global_mut::<Session>();
    session.drag.cancel();
    for panel_id in panels {
        session.panels.remove(&panel_id);
    }
    close_window(id, cx);
    changed(cx);
    record(cx);
    save_now(cx);
    Ok(())
}
