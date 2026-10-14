//! Native workspaces, persistent dock trees and cross-window panel ownership.
use crate::{
    board_bar::BoardBar,
    boards::{self, BoardSetFile, SavedBoard},
    contents::{ContentEvent, PanelContent},
    model::{PanelKind, PanelModel},
    storage::{self, SavedPanel, SavedWindow, WorkspaceFile},
    transfer::{self, DockSide, DragCoordinator, ScreenPoint, TransferIntent},
};
use gpui_kit::{
    component::{
        Disableable, Placement, Sizable, Theme, ThemeMode,
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
    boards: Option<BoardSetFile>,
    board_histories: HashMap<u64, Vec<WorkspaceFile>>,
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
pub(crate) fn live_panel_ids(cx: &App) -> Vec<u64> {
    // Include stale weak entries so regression checks also catch registry leaks.
    let mut ids: Vec<_> = cx.global::<Session>().panels.keys().copied().collect();
    ids.sort_unstable();
    ids
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
                    let entity_id = cx.entity_id();
                    cx.defer(move |cx| {
                        if panel(id, cx).is_some_and(|p| p.entity_id() == entity_id) {
                            select_symbol(id, symbol, cx);
                        }
                    });
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
        let owner = self.owner;
        let entity_id = cx.entity_id();
        cx.defer(move |cx| {
            let remove = cx
                .global::<Session>()
                .panels
                .get(&id)
                .is_some_and(|p| p.entity_id() == entity_id);
            changed(cx);
            if remove {
                cx.global_mut::<Session>().panels.remove(&id);
                close_if_empty(owner, cx);
                record(cx);
                save_now(cx);
            }
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
                            .child("⠿ 拖动")
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
        cx: &mut Context<Self>,
    ) -> PopupMenu {
        let id = self.model.id;
        let owner = self.owner;
        let locked = is_locked(owner, cx);
        menu = menu
            .item(
                PopupMenuItem::new("弹出为独立窗口")
                    .disabled(locked)
                    .on_click(move |_, _, cx| {
                        cx.defer(move |cx| {
                            let r = detach(id, None, cx);
                            report(r, cx);
                        });
                    }),
            )
            .item(
                PopupMenuItem::new("复制为独立组件")
                    .disabled(locked)
                    .on_click(move |_, _, cx| {
                        cx.defer(move |cx| {
                            let r = duplicate(id, cx);
                            report(r, cx);
                        });
                    }),
            );
        if owner != 0 {
            menu = menu.item(
                PopupMenuItem::new("返回主窗口（标签）")
                    .disabled(locked || is_locked(0, cx))
                    .on_click(move |_, _, cx| {
                        cx.defer(move |cx| {
                            let r = move_between(id, 0, None, cx);
                            report(r, cx);
                        });
                    }),
            );
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = self.owner;
        let id = self.model.id;
        let locked = is_locked(owner, cx);
        let focus = self.focus.clone();
        // This is an application drag surface, separate from the native titlebar
        // (which moves the OS window and does not dispatch our docking events).
        let heading = div()
            .h(px(26.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(rgb(0xe8ecf0))
            .bg(rgb(0xf5f6f8))
            .child(
                div()
                    .id(format!("panel-drag-{id}"))
                    .test_support()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(px(11.))
                    .child(div().truncate().child(format!(
                        "{} {}",
                        self.model.symbol.code(),
                        self.model.symbol.name()
                    )))
                    .when(!locked && cross_drag_available(), |d| {
                        d.cursor_grab()
                            .child(div().text_color(rgb(0x86929f)).child("⠿"))
                            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                window.focus(&focus, cx);
                                let point = transfer::screen_point(window, event.position);
                                cx.global_mut::<Session>().drag.begin(owner, id, point);
                                cx.stop_propagation();
                            })
                    }),
            )
            .when(owner != 0, |d| {
                d.child(
                    Button::new(format!("insert-panel-{id}"))
                        .label("插入面板 ▾")
                        .xsmall()
                        .ghost()
                        .disabled(locked)
                        .dropdown_menu(move |menu, window, cx| {
                            insertion_menu(menu, id, window, cx)
                        }),
                )
            });
        div()
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .flex()
            .flex_col()
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
            .child(heading)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.content.clone()),
            )
    }
}

fn is_locked(owner: u64, cx: &App) -> bool {
    workspace(owner, cx).is_some_and(|w| w.read(cx).dock.read(cx).is_locked())
}

/// Exact placement is available on every platform, including Wayland and when
/// a native floating window physically obscures the desired destination.
fn insertion_menu(
    mut menu: PopupMenu,
    source: u64,
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    let mut targets: Vec<_> = cx
        .global::<Session>()
        .panels
        .values()
        .filter_map(WeakEntity::upgrade)
        .filter_map(|p| {
            let p = p.read(cx);
            (p.model.id != source).then_some((p.owner, p.model.id, p.model.kind, p.model.symbol))
        })
        .collect();
    targets.sort_by_key(|(owner, id, _, _)| (*owner, *id));
    for (owner, id, kind, symbol) in targets {
        let title = format!(
            "{} · {} {} #{}",
            if owner == 0 {
                "主窗口".to_owned()
            } else {
                format!("窗口 {owner}")
            },
            kind.title(),
            symbol.code(),
            id
        );
        if is_locked(owner, cx) {
            menu = menu.item(PopupMenuItem::new(format!("{title}（已锁定）")).disabled(true));
            continue;
        }
        menu = menu.submenu(title, window, cx, move |mut menu, _, _| {
            for (label, side) in [
                ("← 插入左侧", DockSide::Left),
                ("→ 插入右侧", DockSide::Right),
                ("↑ 插入上方", DockSide::Top),
                ("↓ 插入下方", DockSide::Bottom),
                ("▣ 合并为标签", DockSide::Center),
            ] {
                menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                    cx.defer(move |cx| {
                        let result = move_between(source, owner, Some((id, side)), cx);
                        report(result, cx);
                    });
                }));
            }
            menu
        });
    }
    let empty_windows: Vec<_> = cx
        .global::<Session>()
        .windows
        .keys()
        .copied()
        .filter(|owner| {
            !cx.global::<Session>()
                .panels
                .values()
                .filter_map(WeakEntity::upgrade)
                .any(|p| p.read(cx).owner == *owner)
        })
        .collect();
    for owner in empty_windows {
        menu = menu.item(
            PopupMenuItem::new(if owner == 0 {
                "插入空白主窗口".to_owned()
            } else {
                format!("插入空白窗口 {owner}")
            })
            .disabled(is_locked(owner, cx))
            .on_click(move |_, _, cx| {
                cx.defer(move |cx| {
                    let result = move_between(source, owner, None, cx);
                    report(result, cx);
                })
            }),
        );
    }
    menu
}
fn cross_drag_available() -> bool {
    cfg!(target_os = "windows") || std::env::var_os("WAYLAND_DISPLAY").is_none()
}

pub(crate) struct Workspace {
    id: u64,
    pub(crate) handle: AnyWindowHandle,
    pub(crate) dock: Entity<DockArea>,
    bounds: [f32; 4],
    board_bar: Option<Entity<BoardBar>>,
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
        window.on_window_should_close(cx, move |_, cx| {
            if id == 0 {
                save_now(cx);
                cx.quit();
                true
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
        Self {
            id,
            handle: window.window_handle(),
            dock,
            bounds: window_bounds(window),
            board_bar: (id == 0).then(|| cx.new(|cx| BoardBar::new(window, cx))),
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
                .child(quote, Some(px(320.)))
                .child(book, Some(px(300.)))
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

fn floating_placement(
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

fn focus_panel(id: u64, cx: &mut App) {
    if let Some(p) = panel(id, cx) {
        let view = p.read(cx);
        let focus = view.focus.clone();
        if let Some(w) = workspace(view.owner, cx) {
            let handle = w.read(cx).handle;
            let _ = handle.update(cx, |_, window, cx| {
                window.activate_window();
                window.focus(&focus, cx);
            });
        }
    }
}

/// All component creation paths use a native, focused destination so the new
/// component cannot silently become an inactive tab in an unrelated group.
fn create_floating(owner: u64, saved: SavedPanel, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(!is_locked(owner, cx), "布局已锁定，请先解锁");
    let (display, bounds) = floating_placement(owner, saved.model.kind, None, cx)?;
    let id = saved.model.id;
    let destination = allocate(cx);
    let w = open_workspace_on_display(destination, None, display, Some(bounds), cx)?;
    let handle = w.read(cx).handle;
    let dock = w.read(cx).dock.clone();
    if let Err(error) = handle.update(cx, |_, window, cx| {
        let p = StockPanel::create(destination, saved, window, cx);
        dock.update(cx, |d, cx| {
            d.add_panel_view(panel_handle(p), DockPlacement::Center, None, window, cx)
        });
    }) {
        close_window(destination, cx);
        return Err(error);
    }
    refresh_market(cx);
    focus_panel(id, cx);
    changed(cx);
    Ok(())
}

pub(crate) fn add_panel(owner: u64, kind: PanelKind, cx: &mut App) -> anyhow::Result<()> {
    anyhow::ensure!(
        cx.global::<Session>()
            .panels
            .values()
            .filter_map(WeakEntity::upgrade)
            .count()
            < 128,
        "最多支持 128 个组件"
    );
    anyhow::ensure!(!is_locked(owner, cx), "布局已锁定，请先解锁");
    anyhow::ensure!(workspace(owner, cx).is_some(), "源窗口已关闭");
    anyhow::ensure!(
        cx.global::<Session>().windows.len() < 16,
        "最多支持 16 个工作区窗口"
    );
    let id = allocate(cx);
    let mut model = PanelModel::new(id, kind);
    // Prefer the invoking workspace's linked market context. This remains
    // deterministic with multiple independent groups and preserves duplication
    // separately through its complete SavedPanel snapshot.
    let context = cx
        .global::<Session>()
        .panels
        .values()
        .filter_map(WeakEntity::upgrade)
        .filter_map(|p| {
            let p = p.read(cx);
            (p.owner == owner && p.model.link_group != 0).then(|| p.model.clone())
        })
        .min_by_key(|p| p.id);
    if let Some(context) = context {
        model.symbol = context.symbol;
        model.link_group = context.link_group;
    }
    create_floating(
        owner,
        SavedPanel {
            model,
            content: serde_json::Value::Null,
        },
        cx,
    )
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
    anyhow::ensure!(!is_locked(owner, cx), "布局已锁定，请先解锁");
    anyhow::ensure!(
        cx.global::<Session>().windows.len() < 16,
        "最多支持 16 个工作区窗口"
    );
    let mut saved = p.read(cx).saved(cx);
    saved.model.id = allocate(cx);
    create_floating(owner, saved, cx)
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
    anyhow::ensure!(
        !is_locked(source, cx) && !is_locked(destination, cx),
        "布局已锁定，请先解锁"
    );
    if let Some((target_id, _)) = target {
        if target_id == id {
            anyhow::ensure!(source == destination, "目标组件不属于目标窗口");
            return Ok(());
        }
        let target_panel = panel(target_id, cx).ok_or_else(|| anyhow::anyhow!("目标组件已关闭"))?;
        anyhow::ensure!(
            target_panel.read(cx).owner == destination,
            "目标组件已移动，请重新选择位置"
        );
    }
    let dest = workspace(destination, cx).ok_or_else(|| anyhow::anyhow!("目标窗口已关闭"))?;
    let dest_handle = dest.read(cx).handle;
    let dest_dock = dest.read(cx).dock.clone();
    let insertion = dock_target(dest_dock.read(cx), target, cx);
    anyhow::ensure!(
        target.is_none() || insertion.is_some(),
        "目标组件不在可停靠区域内"
    );
    if source == destination {
        if let Some(insertion) = insertion {
            dest_handle.update(cx, |_, window, cx| {
                dest_dock.update(cx, |d, cx| {
                    d.move_panel(PanelId::from(p.entity_id()), insertion, window, cx)
                })
            })?;
        }
        focus_panel(id, cx);
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
    refresh_market(cx);
    focus_panel(id, cx);
    changed(cx);
    close_if_empty(source, cx);
    Ok(())
}
pub(crate) fn detach(id: u64, position: Option<ScreenPoint>, cx: &mut App) -> anyhow::Result<()> {
    let source = panel(id, cx).ok_or_else(|| anyhow::anyhow!("组件已关闭"))?;
    let owner = source.read(cx).owner;
    anyhow::ensure!(!is_locked(owner, cx), "布局已锁定，请先解锁");
    let kind = source.read(cx).model.kind;
    let (display_id, bounds) = floating_placement(owner, kind, position, cx)?;
    let win = allocate(cx);
    // Start with an empty live dock. Loading DockAreaState::default() would
    // deserialize an invalid unnamed placeholder as an extra tab.
    open_workspace_on_display(win, None, display_id, Some(bounds), cx)?;
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

#[derive(Clone, Copy)]
pub(crate) enum BoardPreset {
    Empty,
    Standard,
    DualChart,
}

pub(crate) fn active_board_id(cx: &App) -> u64 {
    cx.global::<Session>()
        .boards
        .as_ref()
        .map_or(1, |state| state.active_board)
}

pub(crate) fn board_summaries(cx: &App) -> Vec<(u64, String)> {
    cx.global::<Session>().boards.as_ref().map_or_else(
        || vec![(1, "默认看板".into())],
        |state| {
            state
                .boards
                .iter()
                .map(|b| (b.id, b.name.clone()))
                .collect()
        },
    )
}

fn active_board_name(cx: &App) -> String {
    let id = active_board_id(cx);
    board_summaries(cx)
        .into_iter()
        .find(|(board, _)| *board == id)
        .map_or_else(|| "默认看板".into(), |(_, name)| name)
}

/// Inactive boards remain durable values; only the selected board owns live
/// windows. Patch the selected value at every snapshot so inputs and viewports
/// are saved even when they did not emit a layout event.
pub(crate) fn snapshot_board_set(cx: &App) -> BoardSetFile {
    let live = snapshot(cx);
    let Some(mut state) = cx.global::<Session>().boards.clone() else {
        return BoardSetFile::from_legacy(live);
    };
    if let Some(board) = state.boards.iter_mut().find(|b| b.id == state.active_board) {
        board.workspace = live;
    }
    state
}

fn keep_main_geometry(state: &mut WorkspaceFile, cx: &App) {
    if let Some(main) = workspace(0, cx)
        && let Some(saved) = state.windows.iter_mut().find(|w| w.id == 0)
    {
        saved.bounds = main.read(cx).bounds;
    }
}

/// Validate and load the complete destination before committing its identity or
/// history. `restore` rolls back all window/panel ownership on a load failure.
fn activate_board_set(mut state: BoardSetFile, cx: &mut App) -> anyhow::Result<()> {
    boards::validate(&state)?;
    let target_id = state.active_board;
    let source_id = active_board_id(cx);
    let target = state
        .boards
        .iter_mut()
        .find(|b| b.id == target_id)
        .expect("validated active board");
    target.workspace.paused = cx.global::<Session>().paused;
    keep_main_geometry(&mut target.workspace, cx);
    let target_workspace = target.workspace.clone();
    record(cx);
    // Allocation never goes backwards, including while restoring an older
    // board or rolling back a failed transition.
    cx.global_mut::<Session>().next_id = cx.global::<Session>().next_id.max(state.next_entity_id());
    restore(target_workspace, cx)?;
    let live = snapshot(cx);
    let session = cx.global_mut::<Session>();
    let old_history = std::mem::take(&mut session.history);
    session.board_histories.insert(source_id, old_history);
    session.history = session
        .board_histories
        .remove(&target_id)
        .unwrap_or_default();
    session.last = Some(live);
    session.boards = Some(state);
    changed(cx);
    Ok(())
}

pub(crate) fn switch_board(id: u64, cx: &mut App) -> anyhow::Result<()> {
    let mut state = snapshot_board_set(cx);
    anyhow::ensure!(state.boards.iter().any(|b| b.id == id), "看板不存在");
    if state.active_board == id {
        return Ok(());
    }
    state.active_board = id;
    activate_board_set(state, cx)?;
    status(format!("已切换到 {}", active_board_name(cx)), cx);
    save_now(cx);
    Ok(())
}

fn unique_board_name(
    name: &str,
    except: Option<u64>,
    state: &BoardSetFile,
) -> anyhow::Result<String> {
    let name = boards::validate_board_name(name)?;
    anyhow::ensure!(
        !state
            .boards
            .iter()
            .any(|b| Some(b.id) != except && b.name == name),
        "已存在同名看板"
    );
    Ok(name)
}

fn insert_board(name: &str, workspace: WorkspaceFile, cx: &mut App) -> anyhow::Result<u64> {
    let mut state = snapshot_board_set(cx);
    let name = unique_board_name(name, None, &state)?;
    anyhow::ensure!(
        state.boards.len() < boards::MAX_BOARDS,
        "最多支持 16 个看板"
    );
    anyhow::ensure!(
        state.next_board_id < storage::MAX_STABLE_ID,
        "看板编号已用尽"
    );
    let id = state.next_board_id;
    state.next_board_id += 1;
    state.boards.push(SavedBoard {
        id,
        name,
        workspace,
    });
    state.active_board = id;
    activate_board_set(state, cx)?;
    status(format!("已创建看板 {}", active_board_name(cx)), cx);
    save_now(cx);
    Ok(id)
}

fn empty_tabs() -> PanelState {
    PanelState {
        panel_name: "TabPanel".into(),
        children: vec![],
        info: PanelInfo::Tabs { active_index: 0 },
    }
}

fn next_saved_id(next_id: &mut u64) -> anyhow::Result<u64> {
    anyhow::ensure!(*next_id < storage::MAX_STABLE_ID, "组件编号已用尽");
    let id = *next_id;
    *next_id += 1;
    Ok(id)
}

fn preset_workspace(
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
        version: storage::SCHEMA_VERSION,
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
    storage::validate(&state)?;
    Ok(state)
}

pub(crate) fn create_board(name: &str, preset: BoardPreset, cx: &mut App) -> anyhow::Result<u64> {
    let state = snapshot_board_set(cx);
    unique_board_name(name, None, &state)?;
    anyhow::ensure!(
        state.boards.len() < boards::MAX_BOARDS,
        "最多支持 16 个看板"
    );
    let bounds = workspace(0, cx)
        .map(|w| w.read(cx).bounds)
        .unwrap_or([40., 40., 1536., 930.]);
    let workspace = preset_workspace(
        preset,
        state.next_entity_id().max(cx.global::<Session>().next_id),
        bounds,
        cx.global::<Session>().paused,
    )?;
    insert_board(name, workspace, cx)
}

/// Copy identities as well as window-local entities. Hidden tabs and side docks
/// participate; payloads of unknown panels stay byte-for-byte equivalent JSON.
fn remap_workspace_ids(state: &mut WorkspaceFile, mut next_id: u64) -> anyhow::Result<()> {
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
    storage::validate(state)
}

pub(crate) fn duplicate_board(name: &str, cx: &mut App) -> anyhow::Result<u64> {
    let state = snapshot_board_set(cx);
    unique_board_name(name, None, &state)?;
    anyhow::ensure!(
        state.boards.len() < boards::MAX_BOARDS,
        "最多支持 16 个看板"
    );
    let mut copied = snapshot(cx);
    remap_workspace_ids(
        &mut copied,
        state.next_entity_id().max(cx.global::<Session>().next_id),
    )?;
    insert_board(name, copied, cx)
}

pub(crate) fn rename_board(id: u64, name: &str, cx: &mut App) -> anyhow::Result<()> {
    let mut state = snapshot_board_set(cx);
    let name = unique_board_name(name, Some(id), &state)?;
    let board = state
        .boards
        .iter_mut()
        .find(|b| b.id == id)
        .ok_or_else(|| anyhow::anyhow!("看板不存在"))?;
    board.name = name;
    cx.global_mut::<Session>().boards = Some(state);
    changed(cx);
    save_now(cx);
    Ok(())
}

pub(crate) fn move_board(id: u64, offset: isize, cx: &mut App) -> anyhow::Result<()> {
    let mut state = snapshot_board_set(cx);
    let current = state
        .boards
        .iter()
        .position(|b| b.id == id)
        .ok_or_else(|| anyhow::anyhow!("看板不存在"))?;
    let target = current
        .saturating_add_signed(offset)
        .min(state.boards.len() - 1);
    if target != current {
        let board = state.boards.remove(current);
        state.boards.insert(target, board);
        cx.global_mut::<Session>().boards = Some(state);
        changed(cx);
        save_now(cx);
    }
    Ok(())
}

pub(crate) fn delete_board(id: u64, cx: &mut App) -> anyhow::Result<()> {
    let mut state = snapshot_board_set(cx);
    anyhow::ensure!(state.boards.len() > 1, "至少保留一个看板");
    let index = state
        .boards
        .iter()
        .position(|b| b.id == id)
        .ok_or_else(|| anyhow::anyhow!("看板不存在"))?;
    if state.active_board == id {
        let neighbor = if index + 1 < state.boards.len() {
            index + 1
        } else {
            index - 1
        };
        state.active_board = state.boards[neighbor].id;
        state.boards.remove(index);
        activate_board_set(state, cx)?;
    } else {
        state.boards.remove(index);
        cx.global_mut::<Session>().boards = Some(state);
    }
    cx.global_mut::<Session>().board_histories.remove(&id);
    changed(cx);
    status("已删除看板", cx);
    save_now(cx);
    Ok(())
}

pub(crate) fn restore_board_set(mut state: BoardSetFile, cx: &mut App) -> anyhow::Result<()> {
    boards::validate(&state)?;
    let target = state
        .boards
        .iter_mut()
        .find(|b| b.id == state.active_board)
        .expect("validated active board");
    keep_main_geometry(&mut target.workspace, cx);
    let workspace = target.workspace.clone();
    let next_id = state.next_entity_id().max(cx.global::<Session>().next_id);
    cx.global_mut::<Session>().next_id = next_id;
    restore(workspace, cx)?;
    let live = snapshot(cx);
    let session = cx.global_mut::<Session>();
    session.boards = Some(state);
    session.board_histories.clear();
    session.history.clear();
    session.last = Some(live);
    changed(cx);
    Ok(())
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
    let state = snapshot_board_set(cx);
    match storage::write_board_set(&path, &state) {
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
                    dock: DockAreaState {
                        version: Some(1),
                        center: empty_tabs(),
                        ..Default::default()
                    },
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
            let min_width = if saved.id == 0 { 1024. } else { 240. };
            let min_height = if saved.id == 0 { 680. } else { 240. };
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

pub(crate) fn restore(state: WorkspaceFile, cx: &mut App) -> anyhow::Result<()> {
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

pub(crate) fn undo(cx: &mut App) {
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
        storage::read(&path).and_then(|mut state| {
            // A template may have originated from another board. Assign fresh
            // identities so its panels cannot alias inactive boards or history.
            let next_id = snapshot_board_set(cx).next_entity_id();
            remap_workspace_ids(&mut state, next_id)?;
            state.paused = cx.global::<Session>().paused;
            keep_main_geometry(&mut state, cx);
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
        let board_name = active_board_name(cx);
        let locked = self.dock.read(cx).is_locked();
        let target = cx
            .global::<Session>()
            .drag
            .target
            .filter(|t| t.window == id && !locked);
        let full_header = div()
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
            .child(div().flex_1().min_w_0().children(self.board_bar.clone()))
            .child(
                Button::new("add-component")
                    .label("＋ 添加组件")
                    .disabled(locked)
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
        let floating_header = div()
            .h(px(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_1()
            .gap_1()
            .border_b_1()
            .border_color(rgb(0xe8ecf0))
            .bg(rgb(0xffffff))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(11.))
                    .text_color(rgb(0x86929f))
                    .child(format!("{} · 独立组件", board_name)),
            )
            .child(
                Button::new("add-component")
                    .label("＋")
                    .xsmall()
                    .ghost()
                    .disabled(locked)
                    .dropdown_menu(move |mut menu, _, _| {
                        for kind in PanelKind::ALL {
                            menu = menu.item(PopupMenuItem::new(kind.title()).on_click(
                                move |_, _, cx| {
                                    cx.defer(move |cx| {
                                        let result = add_panel(id, kind, cx);
                                        report(result, cx);
                                    })
                                },
                            ));
                        }
                        menu
                    }),
            )
            .child(
                Button::new("floating-layout-lock")
                    .label(if locked { "解锁" } else { "锁定" })
                    .xsmall()
                    .ghost()
                    .on_click(move |_, window, cx| {
                        if let Some(w) = workspace(id, cx) {
                            let dock = w.read(cx).dock.clone();
                            dock.update(cx, |d, cx| d.set_locked(!locked, window, cx));
                        }
                        changed(cx);
                    }),
            );
        let owned: Vec<_> = cx
            .global::<Session>()
            .panels
            .values()
            .filter_map(WeakEntity::upgrade)
            .filter_map(|p| {
                let p = p.read(cx);
                (p.owner == id).then(|| p.model.clone())
            })
            .collect();
        let panel_count = owned.len();
        if id == 0 {
            window.set_window_title(&format!("Tydaro · {board_name}"));
        } else {
            let title = if let [model] = owned.as_slice() {
                format!(
                    "{} {} · {} · {} · Tydaro",
                    model.symbol.code(),
                    model.symbol.name(),
                    model.kind.title(),
                    board_name
                )
            } else {
                format!("Tydaro · {board_name} · 独立工作区")
            };
            window.set_window_title(&title);
        }
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
                cx.global_mut::<Session>()
                    .drag
                    .set_window_panel_count(id, panel_count);
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
            .when(id == 0, |d| d.child(full_header))
            .when(id != 0, |d| d.child(floating_header))
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
                            .child(self.dock.clone())
                            .when(id == 0 && panel_count == 0, |d| {
                                d.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .justify_center()
                                        .gap_2()
                                        .text_color(rgb(0x86929f))
                                        .child(div().text_size(px(16.)).child("自由搭配你的看板"))
                                        .child("从右上角「添加组件」开始，拖动组件标题插入此处"),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .h(px(if id == 0 { 27. } else { 22. }))
                    .flex_shrink_0()
                    .min_w_0()
                    .overflow_hidden()
                    .border_t_1()
                    .border_color(rgb(0xe4e8ee))
                    .flex()
                    .items_center()
                    .px_2()
                    .gap_3()
                    .text_size(px(11.))
                    .when(id == 0, |d| {
                        d.child(div().text_color(rgb(0x009b9a)).child("● 演示行情"))
                            .child("恒生指数  24,345.20   +0.30%     纳斯达克  23,642.17   +0.86%")
                            .child(div().flex_1())
                    })
                    .child(
                        div()
                            .when(id != 0, |d| d.flex_1())
                            .min_w_0()
                            .truncate()
                            .child(if !message.is_empty() {
                                message
                            } else if id != 0 {
                                "模拟 · 拖动股票标题 / 插入面板".to_owned()
                            } else {
                                format!("拖动股票标题或 ⠿ 停靠 · 自动保存  |  {tick:04}")
                            }),
                    ),
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
                        .rounded_sm()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .bg(rgb(0xffffff))
                                .text_color(rgb(0x009b9a))
                                .child(match t.side {
                                    DockSide::Left => "松开插入左侧",
                                    DockSide::Right => "松开插入右侧",
                                    DockSide::Top => "松开插入上方",
                                    DockSide::Bottom => "松开插入下方",
                                    DockSide::Center => "松开合并为标签",
                                }),
                        ),
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
        boards: None,
        board_histories: HashMap::new(),
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
                match storage::read_board_set(&path) {
                    Ok(state) => Some(state),
                    Err(error) => {
                        status(format!("布局读取失败，已使用默认布局：{error}"), cx);
                        None
                    }
                }
            } else {
                None
            };
            if let Some(state) = saved {
                let result = restore_board_set(state, cx);
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
    use super::{
        BoardPreset, geometry_changed, layout_key, preserve_live_state, preset_workspace,
        remap_workspace_ids, saved_panel_ids,
    };
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
    fn board_presets_have_valid_dock_trees_and_bounded_fresh_identities() {
        for (preset, count) in [
            (BoardPreset::Empty, 0),
            (BoardPreset::Standard, 9),
            (BoardPreset::DualChart, 10),
        ] {
            let state = preset_workspace(preset, 100, [40., 40., 1536., 930.], true).unwrap();
            crate::storage::validate(&state).unwrap();
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
                crate::storage::MAX_STABLE_ID - 1,
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
