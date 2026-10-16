//! Workspaces and toolbar rendering, docking hit areas and drag preview.
use super::transfer::{self, DockSide};
use super::{
    Session, Workspace, active_board_name, add_panel, changed, perform_transfer, report, reset,
    template, undo, workspace,
};
use crate::model::PanelKind;
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        dock::*,
        menu::{DropdownMenu, PopupMenuItem},
    },
    prelude::*,
    *,
};

pub(crate) fn market_toolbar(
    board_bar: &Option<AnyView>,
    dock: &Entity<DockArea>,
    cx: &App,
) -> AnyElement {
    let id = 0;
    let paused = cx.global::<Session>().paused;
    let locked = dock.read(cx).is_locked();
    div()
        .h(px(46.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .px_4()
        .gap_3()
        .border_b_1()
        .border_color(rgb(0xe8ecf0))
        .bg(rgb(0xffffff))
        .child(div().flex_1().min_w_0().children(board_bar.clone()))
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
                    .item(PopupMenuItem::new("撤销布局修改").on_click(|_, _, cx| cx.defer(undo)))
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
        )
        .into_any_element()
}

pub(crate) fn register_main_dock(bounds: Bounds<Pixels>, window: &Window, cx: &mut App) {
    let rect = transfer::screen_rect(window, bounds);
    cx.global_mut::<Session>().drag.register_window(0, rect);
}
impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let id = self.id;
        let message = cx.global::<Session>().status.clone();
        let board_name = active_board_name(cx);
        let locked = self.dock.read(cx).is_locked();
        let target = cx
            .global::<Session>()
            .drag
            .target
            .filter(|t| t.window == id && !locked);
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
                    "{} · {} {} · {} · Tydaro",
                    model.title(),
                    model.symbol.code(),
                    model.symbol.name(),
                    board_name
                )
            } else {
                format!("Tydaro · {board_name} · 独立工作区")
            };
            window.set_window_title(&title);
        }
        let pre = canvas(
            move |_, _, cx| {
                if id == 0 && !(cx.global::<Session>().host.market_visible)(cx) {
                    cx.global_mut::<Session>().drag.remove_window(0);
                    return;
                }
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
            .when_some(self.main_content.clone(), |d, content| d.child(content))
            .when(id != 0, |d| {
                d.child(floating_header)
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
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
                                .inset_0()
                                .size_full(),
                            )
                            .child(self.dock.clone()),
                    )
                    .child(
                        div()
                            .h(px(22.))
                            .flex_shrink_0()
                            .border_t_1()
                            .border_color(rgb(0xe4e8ee))
                            .px_2()
                            .text_size(px(11.))
                            .child(if message.is_empty() {
                                "模拟 · 拖动股票标题 / 插入面板".to_owned()
                            } else {
                                message
                            }),
                    )
            })
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
