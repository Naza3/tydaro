//! Persistent panel identity adapted to GPUI Dock panels and window-local inputs.
use super::{
    Session, changed, close_if_empty, detach, duplicate, move_between, panel, record,
    refresh_market, report, save_now, select_symbol, workspace,
};
use super::{
    snapshot::SavedPanel,
    transfer::{self, DockSide},
};
use crate::model::PanelModel;
use crate::panels::{ContentEvent, PanelContent};
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        dock::*,
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu, PopupMenu, PopupMenuItem},
    },
    prelude::*,
    *,
};

pub(super) struct StockPanel {
    pub(super) model: PanelModel,
    pub(super) owner: u64,
    pub(super) content: Entity<PanelContent>,
    name_input: Entity<InputState>,
    naming: bool,
    name_error: Option<String>,
    pub(super) focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
impl StockPanel {
    pub(super) fn create(
        owner: u64,
        saved: SavedPanel,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<Entity<Self>> {
        crate::panels::snapshot::validate(&saved.content)?;
        let model = saved.model;
        let id = model.id;
        let entity = cx.new(|cx: &mut Context<Self>| {
            let content =
                cx.new(|cx| PanelContent::new(model.kind, model.symbol, model.period, window, cx));
            content
                .update(cx, |content, cx| {
                    content.restore(&saved.content, window, cx)
                })
                .expect("panel content was validated before constructing native state");
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
                ContentEvent::ListChanged => {
                    let id = this.model.id;
                    let entity_id = cx.entity_id();
                    cx.defer(move |cx| {
                        if panel(id, cx).is_some_and(|p| p.entity_id() == entity_id) {
                            changed(cx);
                            record(cx);
                            save_now(cx);
                        }
                    });
                }
            });
            let name_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("组件名称（留空恢复默认）"));
            let name_subscription = cx.subscribe(&name_input, |this, _, event, cx| match event {
                InputEvent::PressEnter { .. } if this.naming => this.confirm_name(cx),
                InputEvent::Change => {
                    this.name_error = None;
                    cx.notify();
                }
                _ => {}
            });
            Self {
                model,
                owner,
                content,
                name_input,
                naming: false,
                name_error: None,
                focus: cx.focus_handle(),
                _subscriptions: vec![sub, name_subscription],
            }
        });
        cx.global_mut::<Session>().next_id = cx.global::<Session>().next_id.max(id + 1);
        cx.global_mut::<Session>()
            .panels
            .insert(id, entity.downgrade());
        Ok(entity)
    }
    pub(super) fn saved(&self, cx: &App) -> SavedPanel {
        SavedPanel {
            model: self.model.clone(),
            content: self.content.read(cx).snapshot(cx),
        }
    }

    fn begin_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.naming = true;
        self.name_error = None;
        self.name_input.update(cx, |input, cx| {
            input.set_value(self.model.title().to_owned(), window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        cx.notify();
    }

    fn cancel_name(&mut self, cx: &mut Context<Self>) {
        self.naming = false;
        self.name_error = None;
        cx.notify();
    }

    fn confirm_name(&mut self, cx: &mut Context<Self>) {
        if !self.naming {
            return;
        }
        match PanelModel::normalize_name(&self.name_input.read(cx).value()) {
            Ok(name) => {
                self.model.custom_name = name;
                self.cancel_name(cx);
                let id = self.model.id;
                let entity_id = cx.entity_id();
                cx.defer(move |cx| {
                    if panel(id, cx).is_some_and(|p| p.entity_id() == entity_id) {
                        changed(cx);
                        save_now(cx);
                    }
                });
            }
            Err(error) => {
                self.name_error = Some(error.to_string());
                cx.notify();
            }
        }
    }

    fn rename_item(owner: WeakEntity<Self>, id: u64) -> PopupMenuItem {
        PopupMenuItem::new("重命名组件").on_click(move |_, window, cx| {
            let owner = owner.clone();
            // Menus restore trigger focus on close. Focus the editor afterward,
            // and ignore a stale callback from a transferred/restored instance.
            window.defer(cx, move |window, cx| {
                if panel(id, cx).is_some_and(|p| p.entity_id() == owner.entity_id()) {
                    let _ = owner.update(cx, |this, cx| this.begin_name(window, cx));
                }
            });
        })
    }

    fn name_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let id = self.model.id;
        div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .p_1()
            .gap_1()
            .border_b_1()
            .border_color(rgb(0xe8ecf0))
            .child(
                Input::new(&self.name_input)
                    .id(format!("panel-name-input-{id}"))
                    .small()
                    .w_full(),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(format!("panel-name-confirm-{id}"))
                            .label("确认")
                            .small()
                            .ghost()
                            .text_color(rgb(0x009b9a))
                            .on_click(cx.listener(|this, _, _, cx| this.confirm_name(cx))),
                    )
                    .child(
                        Button::new(format!("panel-name-cancel-{id}"))
                            .label("取消")
                            .small()
                            .ghost()
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_name(cx))),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(0x85909f))
                            .child("留空恢复默认"),
                    ),
            )
            .when_some(self.name_error.clone(), |this, error| {
                this.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(0xef4770))
                        .child(error),
                )
            })
            .into_any_element()
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
        let title = self.model.title().to_owned();
        div()
            .id(format!("panel-title-{}", self.model.id))
            .aria_label(title.clone())
            .test_support()
            .truncate()
            .child(title)
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
            .item(Self::rename_item(cx.weak_entity(), id))
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
        let rename_owner = cx.weak_entity();
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
            })
            .child(
                Button::new(format!("panel-menu-{id}"))
                    .label("⋯")
                    .xsmall()
                    .ghost()
                    .dropdown_menu(move |menu, _, _| {
                        menu.item(Self::rename_item(rename_owner.clone(), id))
                    }),
            );
        div()
            .id(format!("stock-panel-{id}"))
            .test_support()
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if this.naming && event.keystroke.key == "escape" {
                    this.cancel_name(cx);
                    cx.stop_propagation();
                }
            }))
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
            .when(self.naming, |this| this.child(self.name_editor(cx)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(self.content.clone()),
            )
    }
}

pub(super) fn is_locked(owner: u64, cx: &App) -> bool {
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
            (p.model.id != source).then(|| {
                (
                    p.owner,
                    p.model.id,
                    p.model.title().to_owned(),
                    p.model.symbol,
                )
            })
        })
        .collect();
    targets.sort_by_key(|(owner, id, _, _)| (*owner, *id));
    for (owner, id, name, symbol) in targets {
        let title = format!(
            "{} · {} {} #{}",
            if owner == 0 {
                "主窗口".to_owned()
            } else {
                format!("窗口 {owner}")
            },
            name,
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
