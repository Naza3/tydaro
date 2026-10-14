//! Native workspaces, persistent dock trees and cross-window panel ownership.
use crate::{
    contents::{ContentEvent, PanelContent},
    model::{PanelKind, PanelModel},
    storage::{self, SavedPanel, SavedWindow, WorkspaceFile},
    transfer::{self, DockSide, DragCoordinator, ScreenPoint, TransferIntent},
};
use gpui_kit::{
    component::{
        Placement, Sizable, Theme, ThemeMode,
        button::{Button, ButtonVariants},
        dock::*,
        menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    },
    prelude::*,
    *,
};
use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    time::Duration,
};

struct Session {
    windows: BTreeMap<u64, WeakEntity<Workspace>>,
    panels: HashMap<u64, WeakEntity<StockPanel>>,
    next_id: u64,
    tick: u64,
    paused: bool,
    dirty: bool,
    loading: bool,
    building_owner: u64,
    path: Option<PathBuf>,
    status: String,
    drag: DragCoordinator,
    history: Vec<WorkspaceFile>,
    last: Option<WorkspaceFile>,
}
impl Global for Session {}
fn allocate(cx: &mut App) -> u64 {
    let s = cx.global_mut::<Session>();
    let id = s.next_id;
    s.next_id += 1;
    id
}
pub(crate) fn workspace(id: u64, cx: &App) -> Option<Entity<Workspace>> {
    cx.global::<Session>().windows.get(&id)?.upgrade()
}

#[cfg(feature = "test-support")]
pub(crate) fn verify_transfer_geometry(cx: &mut App) -> anyhow::Result<()> {
    let windows: Vec<_> = cx
        .global::<Session>()
        .windows
        .values()
        .filter_map(WeakEntity::upgrade)
        .collect();
    for workspace in windows {
        let view = workspace.read(cx);
        let id = view.id;
        let handle = view.handle;
        let dock = view.dock.clone();
        handle.update(cx, |_, window, cx| {
            let expected = transfer::screen_rect(window, dock.read(cx).bounds());
            cx.global::<Session>()
                .drag
                .verify_window_geometry(id, expected)
        })??;
    }
    Ok(())
}
fn panel(id: u64, cx: &App) -> Option<Entity<StockPanel>> {
    cx.global::<Session>().panels.get(&id)?.upgrade()
}
fn status(message: impl Into<String>, cx: &mut App) {
    cx.global_mut::<Session>().status = message.into();
    cx.refresh_windows();
}
fn changed(cx: &mut App) {
    if !cx.global::<Session>().loading {
        cx.global_mut::<Session>().dirty = true;
    }
    cx.refresh_windows();
}
fn report(result: anyhow::Result<()>, cx: &mut App) {
    if let Err(error) = result {
        status(format!("操作失败：{error}"), cx);
    }
}
fn refresh_market(cx: &mut App) {
    let tick = cx.global::<Session>().tick;
    let panels: Vec<_> = cx
        .global::<Session>()
        .panels
        .values()
        .filter_map(WeakEntity::upgrade)
        .collect();
    for p in panels {
        p.update(cx, |p, cx| {
            p.content.update(cx, |c, cx| {
                c.set_market(p.model.symbol, p.model.period, tick, cx)
            });
            cx.notify();
        });
    }
}
fn select_symbol(id: u64, symbol: crate::market::Symbol, cx: &mut App) {
    let Some(source) = panel(id, cx) else { return };
    let group = source.read(cx).model.link_group;
    let peers: Vec<_> = cx
        .global::<Session>()
        .panels
        .values()
        .filter_map(WeakEntity::upgrade)
        .filter(|p| {
            p.read(cx).model.id == id || (group != 0 && p.read(cx).model.link_group == group)
        })
        .collect();
    for p in peers {
        p.update(cx, |p, _| p.model.symbol = symbol);
    }
    refresh_market(cx);
    changed(cx);
}

struct StockPanel {
    model: PanelModel,
    owner: u64,
    content: Entity<PanelContent>,
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl StockPanel {
    fn create(owner: u64, saved: SavedPanel, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let model = saved.model;
        let id = model.id;
        let entity = cx.new(|cx: &mut Context<Self>| {
            let content =
                cx.new(|cx| PanelContent::new(model.kind, model.symbol, model.period, window, cx));
            content.update(cx, |content, cx| {
                content.restore(&saved.content, window, cx)
            });
            let sub = cx.subscribe(&content, |this, _, event, cx| match *event {
                ContentEvent::SymbolSelected(symbol) => {
                    let id = this.model.id;
                    cx.defer(move |cx| select_symbol(id, symbol, cx));
                }
                ContentEvent::PeriodChanged(period) => {
                    this.model.period = period;
                    cx.defer(|cx| {
                        refresh_market(cx);
                        changed(cx);
                    });
                }
                ContentEvent::OrderSubmitted => cx.defer(refresh_market),
            });
            Self {
                model,
                owner,
                content,
                focus: cx.focus_handle(),
                _subscriptions: vec![sub],
            }
        });
        cx.global_mut::<Session>().next_id = cx.global::<Session>().next_id.max(id + 1);
        cx.global_mut::<Session>()
            .panels
            .insert(id, entity.downgrade());
        entity
    }
    fn saved(&self, cx: &App) -> SavedPanel {
        SavedPanel {
            model: self.model.clone(),
            content: self.content.read(cx).snapshot(cx),
        }
    }
}
impl EventEmitter<PanelEvent> for StockPanel {}
impl Focusable for StockPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for StockPanel {
    fn panel_name(&self) -> &'static str {
        "tydaro.panel"
    }
    fn dump(&self, cx: &App) -> PanelState {
        PanelState {
            panel_name: self.panel_name().into(),
            children: vec![],
            info: PanelInfo::Panel(
                serde_json::to_value(self.saved(cx)).expect("serializable panel"),
            ),
        }
    }
    fn on_removed(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let id = self.model.id;
        let entity_id = cx.entity_id();
        cx.defer(move |cx| {
            let remove = cx
                .global::<Session>()
                .panels
                .get(&id)
                .is_some_and(|p| p.entity_id() == entity_id);
            if remove {
                cx.global_mut::<Session>().panels.remove(&id);
            }
            changed(cx);
        });
    }
}
impl Panel for StockPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.model.kind.title()
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
    fn zoom_control(&self, _: &App) -> Option<PanelControl> {
        Some(PanelControl::Both)
    }
    fn title_suffix(&mut self, _: &mut Window, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let id = self.model.id;
        let owner = self.owner;
        let group = self.model.link_group;
        let focus = self.focus.clone();
        let locked = workspace(owner, cx).is_some_and(|w| w.read(cx).dock.read(cx).is_locked());
        Some(
            div()
                .flex()
                .gap_2()
                .items_center()
                .text_xs()
                .text_color(rgb(0x009b9a))
                .child(if group == 0 {
                    "独立".into()
                } else {
                    format!("联动 {group}")
                })
                .when(!locked && cross_drag_available(), |d| {
                    d.child(
                        div()
                            .id(("transfer-grip", id))
                            .px_2()
                            .cursor_grab()
                            .child("⠿")
                            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                window.focus(&focus, cx);
                                let point = transfer::screen_point(window, event.position);
                                cx.global_mut::<Session>().drag.begin(owner, id, point);
                                cx.stop_propagation();
                            }),
                    )
                }),
        )
    }
    fn dropdown_menu(
        &mut self,
        mut menu: PopupMenu,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> PopupMenu {
        let id = self.model.id;
        let owner = self.owner;
        menu = menu
            .item(
                PopupMenuItem::new("弹出为独立窗口").on_click(move |_, _, cx| {
                    cx.defer(move |cx| {
                        let r = detach(id, None, cx);
                        report(r, cx);
                    });
                }),
            )
            .item(PopupMenuItem::new("复制此组件").on_click(move |_, _, cx| {
                cx.defer(move |cx| {
                    let r = duplicate(id, cx);
                    report(r, cx);
                });
            }));
        if owner != 0 {
            menu = menu.item(PopupMenuItem::new("返回主窗口").on_click(move |_, _, cx| {
                cx.defer(move |cx| {
                    let r = move_between(id, 0, None, cx);
                    report(r, cx);
                });
            }));
        }
        for group in 0..=3u8 {
            menu = menu.item(
                PopupMenuItem::new(if group == 0 {
                    "取消股票联动".into()
                } else {
                    format!("加入联动组 {group}")
                })
                .on_click(move |_, _, cx| {
                    if let Some(p) = panel(id, cx) {
                        p.update(cx, |p, cx| {
                            p.model.link_group = group;
                            cx.notify();
                        });
                        changed(cx);
                    }
                }),
            );
        }
        menu
    }
}
impl Render for StockPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let owner = self.owner;
        let id = self.model.id;
        div()
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .overflow_hidden()
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let rect = transfer::screen_rect(window, bounds);
                        cx.global_mut::<Session>()
                            .drag
                            .register_panel(owner, id, rect);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .child(self.content.clone())
    }
}
fn cross_drag_available() -> bool {
    cfg!(target_os = "windows") || std::env::var_os("WAYLAND_DISPLAY").is_none()
}

pub(crate) struct Workspace {
    id: u64,
    pub(crate) handle: AnyWindowHandle,
    pub(crate) dock: Entity<DockArea>,
    bounds: [f32; 4],
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
        window.on_window_should_close(cx, move |_, cx| {
            if id == 0 {
                save_now(cx);
                cx.quit();
                true
            } else {
                cx.defer(move |cx| {
                    let r = return_window(id, cx);
                    report(r, cx);
                });
                false
            }
        });
        Self {
            id,
            handle: window.window_handle(),
            dock,
            bounds: window_bounds(window),
            _subscriptions: vec![sub, bounds_sub, active],
        }
    }
}
fn window_bounds(window: &Window) -> [f32; 4] {
    let b = window.bounds();
    [
        b.origin.x.into(),
        b.origin.y.into(),
        b.size.width.into(),
        b.size.height.into(),
    ]
}
fn default_layout(
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
        );
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
        );
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
                .child(quote, Some(px(292.)))
                .child(book, Some(px(220.)))
                .child(trades, None),
            Some(px(320.)),
        );
    dock.update(cx, |d, cx| d.set_center(layout, window, cx));
}
pub(crate) fn open_workspace(
    id: u64,
    saved: Option<SavedWindow>,
    cx: &mut App,
) -> anyhow::Result<Entity<Workspace>> {
    open_workspace_on_display(id, saved, None, None, cx)
}
fn open_workspace_on_display(
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
                px(if id == 0 { 1024. } else { 420. }),
                px(if id == 0 { 680. } else { 320. }),
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
        size(px(b[2].max(420.)), px(b[3].max(320.))),
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
fn add_panel(owner: u64, kind: PanelKind, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(
        cx.global::<Session>()
            .panels
            .values()
            .filter_map(WeakEntity::upgrade)
            .count()
            < 128,
        "最多支持 128 个组件"
    );
    let w = workspace(owner, cx).ok_or_else(|| anyhow::anyhow!("窗口已关闭"))?;
    let handle = w.read(cx).handle;
    let dock = w.read(cx).dock.clone();
    let id = allocate(cx);
    handle.update(cx, |_, window, cx| {
        let p = StockPanel::create(
            owner,
            SavedPanel {
                model: PanelModel::new(id, kind),
                content: serde_json::Value::Null,
            },
            window,
            cx,
        );
        dock.update(cx, |d, cx| {
            d.add_panel_view(panel_handle(p), DockPlacement::Center, None, window, cx)
        });
    })?;
    changed(cx);
    Ok(())
}
fn duplicate(id: u64, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(
        cx.global::<Session>()
            .panels
            .values()
            .filter_map(WeakEntity::upgrade)
            .count()
            < 128,
        "最多支持 128 个组件"
    );
    let p = panel(id, cx).ok_or_else(|| anyhow::anyhow!("组件已关闭"))?;
    let owner = p.read(cx).owner;
    let mut saved = p.read(cx).saved(cx);
    saved.model.id = allocate(cx);
    let w = workspace(owner, cx).unwrap();
    let handle = w.read(cx).handle;
    let dock = w.read(cx).dock.clone();
    handle.update(cx, |_, window, cx| {
        let copy = StockPanel::create(owner, saved, window, cx);
        dock.update(cx, |d, cx| {
            d.add_panel_view(panel_handle(copy), DockPlacement::Center, None, window, cx)
        });
    })?;
    changed(cx);
    Ok(())
}
fn dock_target(dock: &DockArea, target: Option<(u64, DockSide)>, cx: &App) -> Option<InsertTarget> {
    let (id, side) = target?;
    let p = panel(id, cx)?;
    let tree = dock.layout(DockPlacement::Center)?;
    let node = tree.find_panel_node(PanelId::from(p.entity_id()))?;
    Some(match side {
        DockSide::Center => InsertTarget::Tabs {
            node,
            ix: None,
            activate: true,
        },
        s => InsertTarget::Split {
            node,
            placement: match s {
                DockSide::Left => Placement::Left,
                DockSide::Right => Placement::Right,
                DockSide::Top => Placement::Top,
                _ => Placement::Bottom,
            },
            size: None,
        },
    })
}
pub(crate) fn move_between(
    id: u64,
    destination: u64,
    target: Option<(u64, DockSide)>,
    cx: &mut App,
) -> anyhow::Result<()> {
    let p = panel(id, cx).ok_or_else(|| anyhow::anyhow!("组件已关闭"))?;
    let source = p.read(cx).owner;
    let dest = workspace(destination, cx).ok_or_else(|| anyhow::anyhow!("目标窗口已关闭"))?;
    let dest_handle = dest.read(cx).handle;
    let dest_dock = dest.read(cx).dock.clone();
    let insertion = dock_target(dest_dock.read(cx), target, cx);
    if source == destination {
        if let Some(insertion) = insertion {
            dest_handle.update(cx, |_, window, cx| {
                dest_dock.update(cx, |d, cx| {
                    d.move_panel(PanelId::from(p.entity_id()), insertion, window, cx)
                })
            })?;
        }
        changed(cx);
        return Ok(());
    }
    let saved = p.read(cx).saved(cx);
    let source_workspace = workspace(source, cx).ok_or_else(|| anyhow::anyhow!("源窗口已关闭"))?;
    let source_handle = source_workspace.read(cx).handle;
    let source_dock = source_workspace.read(cx).dock.clone();
    // Construct the destination before removing the source. The new entity has
    // window-local input/focus handles and the same durable panel identifier.
    let moved = dest_handle.update(cx, |_, window, cx| {
        let moved = StockPanel::create(destination, saved, window, cx);
        dest_dock.update(cx, |d, cx| {
            d.add_panel_view(
                panel_handle(moved.clone()),
                DockPlacement::Center,
                None,
                window,
                cx,
            );
            if let Some(insertion) = insertion {
                d.move_panel(PanelId::from(moved.entity_id()), insertion, window, cx);
            }
        });
        moved
    })?;
    if let Err(err) = source_handle.update(cx, |_, window, cx| {
        source_dock.update(cx, |d, cx| d.remove_panel(p.clone(), window, cx))
    }) {
        let _ = dest_handle.update(cx, |_, window, cx| {
            dest_dock.update(cx, |d, cx| d.remove_panel(moved, window, cx))
        });
        cx.global_mut::<Session>().panels.insert(id, p.downgrade());
        return Err(err);
    }
    changed(cx);
    close_if_empty(source, cx);
    Ok(())
}
pub(crate) fn detach(id: u64, position: Option<ScreenPoint>, cx: &mut App) -> anyhow::Result<()> {
    let source = panel(id, cx).ok_or_else(|| anyhow::anyhow!("组件已关闭"))?;
    let owner = source.read(cx).owner;
    let source_handle = workspace(owner, cx)
        .ok_or_else(|| anyhow::anyhow!("源窗口已关闭"))?
        .read(cx)
        .handle;
    let source_scale = source_handle.update(cx, |_, window, _| window.scale_factor())?;
    let win = allocate(cx);
    let placement =
        position.map(|position| transfer::detached_window_origin(position, source_scale, cx));
    let display_id = placement.and_then(|(_, display)| display);
    let bounds = placement.map(|(origin, _)| [origin.x.as_f32(), origin.y.as_f32(), 760., 570.]);
    // Start with an empty live dock. Loading DockAreaState::default() would
    // deserialize an invalid unnamed placeholder as an extra tab.
    open_workspace_on_display(win, None, display_id, bounds, cx)?;
    if let Err(error) = move_between(id, win, None, cx) {
        close_window(win, cx);
        return Err(error);
    }
    Ok(())
}
fn close_window(id: u64, cx: &mut App) {
    let handle = workspace(id, cx).map(|w| w.read(cx).handle);
    // Remove stale registry entries even if a native window already vanished.
    cx.global_mut::<Session>().windows.remove(&id);
    cx.global_mut::<Session>().drag.remove_window(id);
    if let Some(handle) = handle {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
}
fn close_if_empty(id: u64, cx: &mut App) {
    if id != 0
        && !cx
            .global::<Session>()
            .panels
            .values()
            .filter_map(WeakEntity::upgrade)
            .any(|p| p.read(cx).owner == id)
    {
        close_window(id, cx);
    }
}
pub(crate) fn return_window(id: u64, cx: &mut App) -> anyhow::Result<()> {
    if id == 0 {
        return Ok(());
    }
    let ids: Vec<_> = cx
        .global::<Session>()
        .panels
        .values()
        .filter_map(WeakEntity::upgrade)
        .filter(|p| p.read(cx).owner == id)
        .map(|p| p.read(cx).model.id)
        .collect();
    for panel in ids {
        move_between(panel, 0, None, cx)?;
    }
    close_window(id, cx);
    changed(cx);
    Ok(())
}
fn perform_transfer(intent: TransferIntent, cx: &mut App) -> anyhow::Result<()> {
    match intent {
        TransferIntent::Dock {
            source_window,
            panel,
            target,
        } => {
            let source =
                workspace(source_window, cx).ok_or_else(|| anyhow::anyhow!("源窗口已关闭"))?;
            let destination =
                workspace(target.window, cx).ok_or_else(|| anyhow::anyhow!("目标窗口已关闭"))?;
            anyhow::ensure!(
                !source.read(cx).dock.read(cx).is_locked()
                    && !destination.read(cx).dock.read(cx).is_locked(),
                "布局已锁定，请先解锁"
            );
            move_between(
                panel,
                target.window,
                target.panel.map(|p| (p, target.side)),
                cx,
            )
        }
        TransferIntent::Detach {
            source_window,
            panel,
            position,
        } => {
            let source =
                workspace(source_window, cx).ok_or_else(|| anyhow::anyhow!("源窗口已关闭"))?;
            anyhow::ensure!(
                !source.read(cx).dock.read(cx).is_locked(),
                "布局已锁定，请先解锁"
            );
            detach(panel, Some(position), cx)
        }
    }
}

pub(crate) fn snapshot(cx: &App) -> WorkspaceFile {
    let session = cx.global::<Session>();
    WorkspaceFile {
        version: storage::SCHEMA_VERSION,
        next_id: session.next_id,
        paused: session.paused,
        windows: session
            .windows
            .values()
            .filter_map(WeakEntity::upgrade)
            .map(|view| {
                let view = view.read(cx);
                let dock = view.dock.read(cx);
                SavedWindow {
                    id: view.id,
                    bounds: view.bounds,
                    locked: dock.is_locked(),
                    dock: dock.dump(cx),
                }
            })
            .collect(),
    }
}

// Undo concerns the workspace arrangement. Market updates, search/form inputs,
// chart viewports, selected stocks and native window moves remain independently
// persisted, but must not fill the layout history once every refresh.
fn layout_key(state: &WorkspaceFile, ignore_sizes: bool) -> serde_json::Value {
    fn normalize(panel: &mut PanelState, ignore_sizes: bool) {
        if panel.panel_name == "tydaro.panel"
            && let PanelInfo::Panel(value) = &mut panel.info
            && let Ok(saved) = serde_json::from_value::<SavedPanel>(value.clone())
        {
            *value = serde_json::json!({
                "id": saved.model.id, "kind": saved.model.kind,
                "link_group": saved.model.link_group,
            });
        }
        if let PanelInfo::Stack { sizes, .. } = &mut panel.info
            && ignore_sizes
        {
            sizes.clear();
        }
        for child in &mut panel.children {
            normalize(child, ignore_sizes);
        }
    }
    let mut state = state.clone();
    state.next_id = 0;
    state.paused = false;
    for window in &mut state.windows {
        window.bounds = [0., 0., 1., 1.];
        normalize(&mut window.dock.center, ignore_sizes);
        for dock in [
            &mut window.dock.left_dock,
            &mut window.dock.right_dock,
            &mut window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            let mut panel = dock.panel().clone();
            normalize(&mut panel, ignore_sizes);
            *dock = DockState::new(
                panel,
                dock.placement(),
                if ignore_sizes { px(0.) } else { dock.size() },
                dock.open(),
            );
        }
    }
    serde_json::to_value(state).expect("workspace is serializable")
}

fn geometry_changed(before: &WorkspaceFile, after: &WorkspaceFile) -> bool {
    before.windows.iter().any(|old| {
        after
            .windows
            .iter()
            .find(|new| new.id == old.id)
            .is_some_and(|new| old.bounds != new.bounds)
    })
}

fn push_history(state: WorkspaceFile, cx: &mut App) {
    let history = &mut cx.global_mut::<Session>().history;
    if history
        .last()
        .is_none_or(|last| layout_key(last, false) != layout_key(&state, false))
    {
        history.push(state);
        if history.len() > 20 {
            history.remove(0);
        }
    }
}

fn record(cx: &mut App) {
    if cx.global::<Session>().loading {
        return;
    }
    let current = snapshot(cx);
    let previous = cx.global_mut::<Session>().last.replace(current.clone());
    if let Some(previous) = previous
        && serde_json::to_value(&previous).ok() != serde_json::to_value(&current).ok()
    {
        cx.global_mut::<Session>().dirty = true;
        // Resizing a native window also rescales its dock slots. Treat that
        // geometry as one native operation, not a new layout undo entry.
        let ignore_sizes = geometry_changed(&previous, &current);
        if layout_key(&previous, ignore_sizes) != layout_key(&current, ignore_sizes) {
            push_history(previous, cx);
        }
    }
}

fn save_now(cx: &mut App) {
    if cx.global::<Session>().loading {
        return;
    }
    let Some(path) = cx.global::<Session>().path.clone() else {
        return;
    };
    let state = snapshot(cx);
    match storage::write(&path, &state) {
        Ok(()) => cx.global_mut::<Session>().dirty = false,
        Err(error) => status(format!("布局保存失败：{error}"), cx),
    }
}

fn saved_panel_ids(state: &WorkspaceFile) -> std::collections::HashSet<u64> {
    fn collect(panel: &PanelState, ids: &mut std::collections::HashSet<u64>) {
        if panel.panel_name == "tydaro.panel"
            && let PanelInfo::Panel(value) = &panel.info
            && let Ok(saved) = serde_json::from_value::<SavedPanel>(value.clone())
        {
            ids.insert(saved.model.id);
        }
        for child in &panel.children {
            collect(child, ids);
        }
    }
    let mut ids = std::collections::HashSet::new();
    for window in &state.windows {
        collect(&window.dock.center, &mut ids);
        for dock in [
            &window.dock.left_dock,
            &window.dock.right_dock,
            &window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            collect(dock.panel(), &mut ids);
        }
    }
    ids
}

fn apply_workspace(state: &WorkspaceFile, cx: &mut App) -> anyhow::Result<()> {
    // Allocate native destinations before replacing any live layout. A newly
    // created window starts empty; its saved panels are constructed exactly once.
    for saved in &state.windows {
        if workspace(saved.id, cx).is_none() {
            open_workspace(
                saved.id,
                Some(SavedWindow {
                    id: saved.id,
                    bounds: saved.bounds,
                    locked: saved.locked,
                    dock: DockAreaState::default(),
                }),
                cx,
            )?;
        }
    }
    for saved in &state.windows {
        let view = workspace(saved.id, cx)
            .ok_or_else(|| anyhow::anyhow!("目标窗口 {} 已关闭", saved.id))?;
        let handle = view.read(cx).handle;
        let dock = view.read(cx).dock.clone();
        cx.global_mut::<Session>().building_owner = saved.id;
        let bounds = handle.update(cx, |_, window, cx| -> anyhow::Result<[f32; 4]> {
            dock.update(cx, |dock, cx| {
                dock.load(saved.dock.clone(), window, cx)?;
                dock.set_locked(saved.locked, window, cx);
                Ok::<_, anyhow::Error>(())
            })?;
            // GPUI exposes resize but no programmatic window-position setter.
            // Reused windows keep their current screen position; fresh windows
            // and the next app launch use the saved, visibility-checked position.
            let min_width = if saved.id == 0 { 1024. } else { 420. };
            let min_height = if saved.id == 0 { 680. } else { 320. };
            let requested = size(
                px(saved.bounds[2].max(min_width)),
                px(saved.bounds[3].max(min_height)),
            );
            if window.bounds().size != requested {
                window.resize(requested);
            }
            Ok(window_bounds(window))
        })??;
        view.update(cx, |view, _| view.bounds = bounds);
    }
    // Retire source windows only after every destination was loaded. The
    // conditional on_removed callback cannot delete a newly registered entity
    // with the same durable panel ID.
    let obsolete: Vec<_> = cx
        .global::<Session>()
        .windows
        .keys()
        .copied()
        .filter(|id| !state.windows.iter().any(|saved| saved.id == *id))
        .collect();
    for id in obsolete {
        close_window(id, cx);
    }
    let ids = saved_panel_ids(state);
    let session = cx.global_mut::<Session>();
    session
        .panels
        .retain(|id, view| ids.contains(id) && view.upgrade().is_some());
    session.next_id = session.next_id.max(state.next_id);
    session.paused = state.paused;
    session.drag = DragCoordinator::default();
    Ok(())
}

fn restore(state: WorkspaceFile, cx: &mut App) -> anyhow::Result<()> {
    storage::validate(&state)?;
    anyhow::ensure!(
        state.windows.iter().any(|window| window.id == 0),
        "布局缺少主窗口"
    );
    let before = snapshot(cx);
    let previous_dirty = cx.global::<Session>().dirty;
    cx.global_mut::<Session>().loading = true;
    let result = apply_workspace(&state, cx);
    if let Err(error) = result {
        let rollback = apply_workspace(&before, cx);
        cx.global_mut::<Session>().loading = false;
        cx.global_mut::<Session>().last = Some(snapshot(cx));
        cx.global_mut::<Session>().dirty = previous_dirty;
        refresh_market(cx);
        return match rollback {
            Ok(()) => Err(anyhow::anyhow!("布局未应用，已恢复原工作区：{error}")),
            Err(rollback_error) => Err(anyhow::anyhow!(
                "布局恢复失败：{error}；回滚也失败：{rollback_error}"
            )),
        };
    }
    cx.global_mut::<Session>().loading = false;
    refresh_market(cx);
    cx.global_mut::<Session>().last = Some(snapshot(cx));
    changed(cx);
    Ok(())
}

// Keep live business/view state when undoing arrangement changes. A panel that
// was actually closed has no live counterpart and recovers its saved content.
fn preserve_live_state(target: &mut WorkspaceFile, current: &WorkspaceFile) {
    fn collect(panel: &PanelState, panels: &mut HashMap<u64, SavedPanel>) {
        if panel.panel_name == "tydaro.panel"
            && let PanelInfo::Panel(value) = &panel.info
            && let Ok(saved) = serde_json::from_value::<SavedPanel>(value.clone())
        {
            panels.insert(saved.model.id, saved);
        }
        for child in &panel.children {
            collect(child, panels);
        }
    }
    fn apply(panel: &mut PanelState, panels: &HashMap<u64, SavedPanel>) {
        if panel.panel_name == "tydaro.panel"
            && let PanelInfo::Panel(value) = &mut panel.info
            && let Ok(mut saved) = serde_json::from_value::<SavedPanel>(value.clone())
            && let Some(live) = panels.get(&saved.model.id)
        {
            saved.model.symbol = live.model.symbol;
            saved.model.period = live.model.period;
            saved.content = live.content.clone();
            *value = serde_json::to_value(saved).expect("saved panel is serializable");
        }
        for child in &mut panel.children {
            apply(child, panels);
        }
    }
    let mut panels = HashMap::new();
    for window in &current.windows {
        collect(&window.dock.center, &mut panels);
        for dock in [
            &window.dock.left_dock,
            &window.dock.right_dock,
            &window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            collect(dock.panel(), &mut panels);
        }
    }
    for window in &mut target.windows {
        if let Some(live) = current.windows.iter().find(|live| live.id == window.id) {
            window.bounds = live.bounds;
        }
        apply(&mut window.dock.center, &panels);
        for dock in [
            &mut window.dock.left_dock,
            &mut window.dock.right_dock,
            &mut window.dock.bottom_dock,
        ]
        .into_iter()
        .flatten()
        {
            let mut panel = dock.panel().clone();
            apply(&mut panel, &panels);
            *dock = DockState::new(panel, dock.placement(), dock.size(), dock.open());
        }
    }
    target.paused = current.paused;
    target.next_id = target.next_id.max(current.next_id);
}

fn undo(cx: &mut App) {
    // Capture a just-completed drag even if the autosave timer has not fired.
    record(cx);
    let Some(mut state) = cx.global::<Session>().history.last().cloned() else {
        status("暂无可撤销的布局修改", cx);
        return;
    };
    preserve_live_state(&mut state, &snapshot(cx));
    match restore(state, cx) {
        Ok(()) => {
            cx.global_mut::<Session>().history.pop();
            status("已撤销布局修改", cx);
        }
        Err(error) => report(Err(error), cx),
    }
}

fn reset(dual: bool, cx: &mut App) {
    record(cx);
    let Some(view) = workspace(0, cx) else {
        status("主窗口不可用，无法切换布局", cx);
        return;
    };
    let before = snapshot(cx);
    let handle = view.read(cx).handle;
    let dock = view.read(cx).dock.clone();
    cx.global_mut::<Session>().loading = true;
    let result = handle.update(cx, |_, window, cx| {
        default_layout(&dock, 0, dual, window, cx);
        dock.update(cx, |dock, cx| dock.set_locked(false, window, cx));
    });
    if let Err(error) = result {
        cx.global_mut::<Session>().loading = false;
        report(Err(error), cx);
        return;
    }
    let secondary: Vec<_> = cx
        .global::<Session>()
        .windows
        .keys()
        .copied()
        .filter(|id| *id != 0)
        .collect();
    for id in secondary {
        close_window(id, cx);
    }
    let current = snapshot(cx);
    let ids = saved_panel_ids(&current);
    cx.global_mut::<Session>()
        .panels
        .retain(|id, view| ids.contains(id) && view.upgrade().is_some());
    cx.global_mut::<Session>().drag = DragCoordinator::default();
    cx.global_mut::<Session>().loading = false;
    push_history(before, cx);
    refresh_market(cx);
    cx.global_mut::<Session>().last = Some(snapshot(cx));
    changed(cx);
}

fn template(save: bool, cx: &mut App) {
    let Some(path) = cx
        .global::<Session>()
        .path
        .as_ref()
        .map(|path| path.with_file_name("template.json"))
    else {
        return;
    };
    let result = if save {
        storage::write(&path, &snapshot(cx))
    } else {
        storage::read(&path).and_then(|state| {
            record(cx);
            let before = snapshot(cx);
            restore(state, cx)?;
            push_history(before, cx);
            Ok(())
        })
    };
    match result {
        Ok(()) => status(
            if save {
                "已保存自定义布局模板"
            } else {
                "已加载自定义布局模板"
            },
            cx,
        ),
        Err(error) => status(format!("模板操作失败：{error}"), cx),
    }
}

#[cfg(feature = "test-support")]
pub(crate) fn verify_workspace_persistence(cx: &mut App) -> anyhow::Result<()> {
    let before = snapshot(cx);
    storage::validate(&before)?;
    let ids = saved_panel_ids(&before);
    restore(before.clone(), cx)?;
    let restored = snapshot(cx);
    anyhow::ensure!(
        before.windows.len() == restored.windows.len(),
        "Restoring changed window count"
    );
    anyhow::ensure!(
        saved_panel_ids(&restored) == ids,
        "Restoring lost panel IDs"
    );
    anyhow::ensure!(
        layout_key(&before, true) == layout_key(&restored, true),
        "Restoring changed the dock tree"
    );
    reset(true, cx);
    undo(cx);
    let undone = snapshot(cx);
    anyhow::ensure!(saved_panel_ids(&undone) == ids, "Undo lost panel IDs");
    anyhow::ensure!(
        layout_key(&before, true) == layout_key(&undone, true),
        "Undo did not restore the multiwindow layout"
    );
    for id in ids {
        anyhow::ensure!(
            panel(id, cx).is_some(),
            "Panel registry lost restored ID {id}"
        );
    }
    Ok(())
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.id;
        let paused = cx.global::<Session>().paused;
        let tick = cx.global::<Session>().tick;
        let message = cx.global::<Session>().status.clone();
        let locked = self.dock.read(cx).is_locked();
        let target = cx
            .global::<Session>()
            .drag
            .target
            .filter(|t| t.window == id && !locked);
        let header = div()
            .h(px(46.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_4()
            .gap_3()
            .border_b_1()
            .border_color(rgb(0xe8ecf0))
            .bg(rgb(0xffffff))
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(19.))
                    .text_color(rgb(0x172736))
                    .child("▥ TYDARO"),
            )
            .child(div().text_xs().text_color(rgb(0x86929f)).child(if id == 0 {
                "股票工作台"
            } else {
                "独立工作区"
            }))
            .child(div().flex_1())
            .child(
                Button::new("add-component")
                    .label("＋ 添加组件")
                    .small()
                    .ghost()
                    .dropdown_menu(move |mut menu, _, _| {
                        for kind in PanelKind::ALL {
                            menu = menu.item(PopupMenuItem::new(kind.title()).on_click(
                                move |_, _, cx| {
                                    cx.defer(move |cx| {
                                        let r = add_panel(id, kind, cx);
                                        report(r, cx);
                                    });
                                },
                            ));
                        }
                        menu
                    }),
            )
            .child(
                Button::new("layout-menu")
                    .label("布局 ▾")
                    .small()
                    .ghost()
                    .dropdown_menu(move |menu, _, _| {
                        menu.item(
                            PopupMenuItem::new("默认行情布局")
                                .on_click(|_, _, cx| cx.defer(|cx| reset(false, cx))),
                        )
                        .item(
                            PopupMenuItem::new("双图对比布局")
                                .on_click(|_, _, cx| cx.defer(|cx| reset(true, cx))),
                        )
                        .item(
                            PopupMenuItem::new("保存为我的模板")
                                .on_click(|_, _, cx| cx.defer(|cx| template(true, cx))),
                        )
                        .item(
                            PopupMenuItem::new("加载我的模板")
                                .on_click(|_, _, cx| cx.defer(|cx| template(false, cx))),
                        )
                        .item(
                            PopupMenuItem::new("撤销布局修改").on_click(|_, _, cx| cx.defer(undo)),
                        )
                        .item(
                            PopupMenuItem::new(if locked {
                                "解锁布局"
                            } else {
                                "锁定布局"
                            })
                            .on_click(move |_, window, cx| {
                                if let Some(w) = workspace(id, cx) {
                                    let dock = w.read(cx).dock.clone();
                                    dock.update(cx, |d, cx| d.set_locked(!locked, window, cx));
                                }
                                changed(cx);
                            }),
                        )
                    }),
            )
            .child(
                Button::new("pause-market")
                    .label(if paused {
                        "▶ 继续演示"
                    } else {
                        "Ⅱ 暂停演示"
                    })
                    .small()
                    .ghost()
                    .on_click(|_, _, cx| {
                        let s = cx.global_mut::<Session>();
                        s.paused = !s.paused;
                        changed(cx);
                    }),
            );
        let nav = div()
            .w(px(120.))
            .flex_shrink_0()
            .h_full()
            .bg(rgb(0xf5f6f8))
            .border_r_1()
            .border_color(rgb(0xe8ecf0))
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .mt_2()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0xe1e7ec))
                    .text_color(rgb(0x203243))
                    .child("▣  个股行情"),
            )
            .child(
                Button::new("nav-watch")
                    .label("☆  自选列表")
                    .ghost()
                    .on_click(move |_, _, cx| {
                        cx.defer(move |cx| {
                            let r = add_panel(id, PanelKind::Watchlist, cx);
                            report(r, cx);
                        })
                    }),
            )
            .child(
                Button::new("nav-charts")
                    .label("◷  多图分析")
                    .ghost()
                    .on_click(|_, _, cx| cx.defer(|cx| reset(true, cx))),
            )
            .child(
                Button::new("nav-trade")
                    .label("⇄  模拟交易")
                    .ghost()
                    .on_click(move |_, _, cx| {
                        cx.defer(move |cx| {
                            let r = add_panel(id, PanelKind::OrderEntry, cx);
                            report(r, cx);
                        })
                    }),
            )
            .child(div().flex_1())
            .child(
                div()
                    .p_2()
                    .text_xs()
                    .text_color(rgb(0x929ba5))
                    .child("FRONTEND PREVIEW\n本地模拟数据\nv0.1.0"),
            );
        let pre = canvas(
            move |_, _, cx| {
                cx.global_mut::<Session>().drag.clear_window_panels(id);
            },
            |_, _, _, _| {},
        )
        .size_0();
        let listener = transfer::window_event_listener(
            |event, window, cx| {
                if cx.global::<Session>().drag.is_active() {
                    if event.pressed_button == Some(MouseButton::Left) {
                        let point = transfer::screen_point(window, event.position);
                        cx.global_mut::<Session>().drag.update(point);
                    } else {
                        cx.global_mut::<Session>().drag.cancel();
                    }
                    cx.refresh_windows();
                }
            },
            |event, window, cx| {
                if event.button == MouseButton::Left && cx.global::<Session>().drag.is_active() {
                    let point = transfer::screen_point(window, event.position);
                    let intent = cx.global_mut::<Session>().drag.finish(point);
                    if let Some(intent) = intent {
                        cx.defer(move |cx| {
                            let r = perform_transfer(intent, cx);
                            report(r, cx);
                        });
                    }
                    cx.refresh_windows();
                }
            },
        );
        let scale = window.scale_factor();
        let origin = window.bounds().origin;
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x233348))
            .text_size(px(13.))
            .on_key_down(|event, _, cx| {
                if event.keystroke.key == "escape" && cx.global::<Session>().drag.is_active() {
                    cx.global_mut::<Session>().drag.cancel();
                    cx.refresh_windows();
                    cx.stop_propagation();
                }
            })
            .child(pre)
            .child(listener)
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .when(id == 0, |d| d.child(nav))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .relative()
                            .child(
                                canvas(
                                    move |bounds, window, cx| {
                                        let rect = transfer::screen_rect(window, bounds);
                                        cx.global_mut::<Session>().drag.register_window(id, rect);
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full(),
                            )
                            .child(self.dock.clone()),
                    ),
            )
            .child(
                div()
                    .h(px(27.))
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(rgb(0xe4e8ee))
                    .flex()
                    .items_center()
                    .px_3()
                    .gap_5()
                    .text_size(px(11.))
                    .child(div().text_color(rgb(0x009b9a)).child("● 演示行情"))
                    .child("恒生指数  24,345.20   +0.30%     纳斯达克  23,642.17   +0.86%")
                    .child(div().flex_1())
                    .child(if message.is_empty() {
                        format!("拖动标签调整布局 · ⠿ 跨窗拖动 · 自动保存  |  {tick:04}")
                    } else {
                        message
                    }),
            )
            .when_some(target, |d, t| {
                let b = t.bounds.preview(t.side);
                d.child(
                    div()
                        .absolute()
                        .left(px(b.x / scale) - origin.x)
                        .top(px(b.y / scale) - origin.y)
                        .w(px(b.width / scale))
                        .h(px(b.height / scale))
                        .bg(rgba(0x00b6b62c))
                        .border_2()
                        .border_color(rgb(0x00a9aa))
                        .rounded_sm(),
                )
            })
    }
}

pub(crate) fn init(cx: &mut App, persistent: bool) {
    gpui_kit::init(cx);
    gpui_kit::component::set_locale("zh-CN");
    Theme::change(ThemeMode::Light, None, cx);
    Theme::global_mut(cx).font_size = px(13.);
    Theme::global_mut(cx).font_family = if cfg!(target_os = "windows") {
        "Microsoft YaHei UI".into()
    } else {
        "Noto Sans CJK SC".into()
    };
    cx.set_global(Session {
        windows: BTreeMap::new(),
        panels: HashMap::new(),
        next_id: 1,
        tick: 0,
        paused: false,
        dirty: false,
        loading: false,
        building_owner: 0,
        path: persistent.then(|| storage::data_dir().join("workspace.json")),
        status: String::new(),
        drag: DragCoordinator::default(),
        history: vec![],
        last: None,
    });
    register_panel(cx, "tydaro.panel", |build, window, cx| {
        let saved = match build.info() {
            PanelInfo::Panel(value) => {
                serde_json::from_value(value.clone()).expect("validated panel payload")
            }
            _ => unreachable!("validated panel info"),
        };
        let owner = cx.global::<Session>().building_owner;
        panel_handle(StockPanel::create(owner, saved, window, cx))
    });
}
pub fn run() {
    #[cfg(feature = "test-support")]
    if let Some(path) = std::env::args().skip_while(|a| a != "--smoke-test").nth(1) {
        if let Err(err) = crate::smoke::run(&path) {
            eprintln!("Smoke test failed: {err:#}");
            std::process::exit(1);
        }
        return;
    }
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            init(cx, true);
            let path = cx.global::<Session>().path.clone().unwrap();
            let saved = if path.exists() {
                match storage::read(&path) {
                    Ok(state) if state.windows.iter().any(|w| w.id == 0) => Some(state),
                    Ok(_) => {
                        status("布局缺少主窗口，已使用默认布局；原文件会保留备份", cx);
                        None
                    }
                    Err(error) => {
                        status(format!("布局读取失败，已使用默认布局：{error}"), cx);
                        None
                    }
                }
            } else {
                None
            };
            if let Some(state) = saved {
                let result = restore(state, cx);
                report(result, cx);
            }
            if workspace(0, cx).is_none()
                && let Err(err) = open_workspace(0, None, cx)
            {
                eprintln!("Cannot open Tydaro: {err:#}");
                cx.quit();
                return;
            }
            cx.global_mut::<Session>().last = Some(snapshot(cx));
            cx.spawn(async move |cx| {
                let mut persistence_ticks = 0_u64;
                loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    persistence_ticks += 1;
                    cx.update(|cx| {
                        let s = cx.global_mut::<Session>();
                        if !s.paused {
                            s.tick += 1;
                        }
                        refresh_market(cx);
                        if cx.global::<Session>().dirty || persistence_ticks.is_multiple_of(5) {
                            record(cx);
                            if cx.global::<Session>().dirty {
                                save_now(cx);
                            }
                        }
                        cx.refresh_windows();
                    });
                }
            })
            .detach();
            cx.activate(true);
        });
}

#[cfg(test)]
mod persistence_tests {
    use super::{geometry_changed, layout_key, preserve_live_state};
    use crate::{
        market::{Period, Symbol},
        model::{PanelKind, PanelModel},
        storage::{SCHEMA_VERSION, SavedPanel, SavedWindow, WorkspaceFile},
    };
    use gpui_kit::{
        component::dock::{DockAreaState, PanelInfo, PanelState},
        px,
    };

    fn fixture() -> WorkspaceFile {
        let panel = SavedPanel {
            model: PanelModel::new(7, PanelKind::Chart),
            content: serde_json::json!({"query": "AAPL", "chart": {"bars_visible": 80}}),
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
    fn history_ignores_business_state_but_detects_layout_and_lock_changes() {
        let original = fixture();
        let mut changed = original.clone();
        changed.next_id = 100;
        changed.paused = true;
        changed.windows[0].bounds = [100., 60., 1600., 1000.];
        let mut saved = payload(&changed);
        saved.model.symbol = Symbol::Msft;
        saved.model.period = Period::Week;
        saved.content = serde_json::json!({"query": "MSFT", "chart": {"bars_visible": 40}});
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
        saved.content = serde_json::json!({"quantity": "500", "chart": {"bars_visible": 120}});
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
        assert_eq!(restored.content, saved.content);
        assert!(historical.paused);
        assert_eq!(historical.next_id, 20);
        assert_eq!(historical.windows[0].bounds, live.windows[0].bounds);
    }
}
