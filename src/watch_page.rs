//! Native monitoring workspace. Quotes and explicitly generated alerts are local demonstrations.
use std::collections::HashSet;

use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Icon, Sizable,
        button::{Button, ButtonVariants},
        checkbox::Checkbox,
        input::{Input, InputEvent, InputState},
    },
    prelude::*,
    *,
};

use crate::{
    desktop_state::{
        self, DesktopEvent, DesktopState, PageId, SourceContext, WatchAlert, WatchGroup, WatchRule,
    },
    desktop_ui::{BG, DOWN, INK, LINE, MUTED, TEAL, UP},
    market::{self, Symbol},
};

const SNAPSHOT_TICK: u64 = 272;
const DATE: &str = "2026-10-14";
const GROUP_COLORS: [u32; 6] = [0x009b9a, 0x6686c0, 0xba9466, 0x9b83ad, 0xc27a87, 0x718e62];

struct GroupEditor {
    id: Option<String>,
    symbols: Vec<Symbol>,
    color: u32,
    deleting: bool,
}

pub struct WatchPage {
    columns: usize,
    actual_columns: usize,
    sorting: u8,
    selected_group: Option<String>,
    selected_symbol: Option<Symbol>,
    unread_only: bool,
    editor: Option<GroupEditor>,
    name: Entity<InputState>,
    note: Entity<InputState>,
    search: Entity<InputState>,
    rules_editor: Option<Vec<WatchRule>>,
    rule_inputs: Vec<Entity<InputState>>,
    error: Option<String>,
    demo_counter: usize,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DesktopEvent> for WatchPage {}

impl WatchPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("例如：科技 · 趋势跟踪"));
        let note = cx.new(|cx| InputState::new(window, cx).placeholder("记录这个分组的关注点"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索名称或代码"));
        let mut subscriptions = Vec::new();
        for input in [&name, &note, &search] {
            subscriptions.push(cx.subscribe(input, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.error = None;
                    cx.notify();
                }
            }));
        }
        subscriptions.push(cx.subscribe(&name, |this, _, event, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.save_group(cx);
            }
        }));
        Self {
            columns: 2,
            actual_columns: 2,
            sorting: 0,
            selected_group: None,
            selected_symbol: None,
            unread_only: false,
            editor: None,
            name,
            note,
            search,
            rules_editor: None,
            rule_inputs: Vec::new(),
            error: None,
            demo_counter: 0,
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn is_editing(&self) -> bool {
        self.editor.is_some() || self.rules_editor.is_some()
    }

    fn begin_group(
        &mut self,
        group: Option<WatchGroup>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (name, note, editor) = if let Some(group) = group {
            (
                group.name,
                group.note,
                GroupEditor {
                    id: Some(group.id),
                    symbols: group.symbols,
                    color: group.color,
                    deleting: false,
                },
            )
        } else {
            (
                String::new(),
                String::new(),
                GroupEditor {
                    id: None,
                    symbols: Vec::new(),
                    color: GROUP_COLORS[0],
                    deleting: false,
                },
            )
        };
        self.editor = Some(editor);
        self.rules_editor = None;
        self.error = None;
        self.name.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
            input.select_all(window, cx);
        });
        self.note
            .update(cx, |input, cx| input.set_value(note, window, cx));
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }

    fn save_group(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.editor else { return };
        let name = self.name.read(cx).value().trim().to_owned();
        let note = self.note.read(cx).value().trim().to_owned();
        let invalid =
            if name.is_empty() || name.chars().count() > 24 || name.chars().any(char::is_control) {
                Some("分组名称需为 1–24 个字符，不能包含控制字符。")
            } else if cx
                .global::<DesktopState>()
                .watch_groups
                .iter()
                .any(|group| Some(&group.id) != editor.id.as_ref() && group.name == name)
            {
                Some("已有同名分组，请使用不同的名称。")
            } else if editor.id.is_none() && cx.global::<DesktopState>().watch_groups.len() >= 128 {
                Some("最多支持 128 个盯盘分组，请先整理已有分组。")
            } else if note.chars().count() > 160 {
                Some("观察备注最多 160 个字符。")
            } else {
                None
            };
        if let Some(error) = invalid {
            self.error = Some(error.into());
            cx.notify();
            return;
        }
        let prior_id = editor.id.clone();
        let symbols = editor.symbols.clone();
        let color = editor.color;
        let count = symbols.len();
        let mut saved_id = String::new();
        desktop_state::update(cx, |state| {
            let id = prior_id.unwrap_or_else(|| state.new_id("watch"));
            saved_id = id.clone();
            let group = WatchGroup {
                id: id.clone(),
                name: name.clone(),
                symbols,
                color,
                note,
            };
            if let Some(existing) = state.watch_groups.iter_mut().find(|group| group.id == id) {
                *existing = group;
            } else {
                state.watch_groups.push(group);
            }
        });
        let index = cx
            .global::<DesktopState>()
            .watch_groups
            .iter()
            .position(|group| group.id == saved_id)
            .unwrap_or(0);
        self.selected_group = Some(saved_id);
        self.scroll.scroll_to_item(1 + index / self.actual_columns);
        self.editor = None;
        self.error = None;
        cx.emit(DesktopEvent::Notice(format!(
            "已保存「{name}」，共 {count} 只股票"
        )));
        cx.notify();
    }

    fn delete_group(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.editor.as_ref().and_then(|editor| editor.id.clone()) else {
            return;
        };
        desktop_state::update(cx, |state| {
            state.watch_groups.retain(|group| group.id != id)
        });
        if self.selected_group.as_ref() == Some(&id) {
            self.selected_group = None;
            self.selected_symbol = None;
        }
        self.editor = None;
        self.error = None;
        cx.emit(DesktopEvent::Notice(
            "已删除盯盘分组，历史异动记录仍保留".into(),
        ));
        cx.notify();
    }

    fn begin_rules(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rules = cx.global::<DesktopState>().rules.clone();
        self.rule_inputs = rules
            .iter()
            .map(|rule| {
                cx.new(|cx| {
                    InputState::new(window, cx).default_value(format!("{:.2}", rule.threshold))
                })
            })
            .collect();
        self.rules_editor = Some(rules);
        self.editor = None;
        self.error = None;
        if let Some(input) = self.rule_inputs.first() {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }

    fn save_rules(&mut self, cx: &mut Context<Self>) {
        let Some(mut rules) = self.rules_editor.clone() else {
            return;
        };
        for (rule, input) in rules.iter_mut().zip(&self.rule_inputs) {
            match input.read(cx).value().trim().parse::<f64>() {
                Ok(value) if value.is_finite() && value > 0. && value <= 100. => {
                    rule.threshold = value
                }
                _ => {
                    self.error = Some("规则阈值请输入大于 0、且不超过 100 的数值。".into());
                    cx.notify();
                    return;
                }
            }
        }
        desktop_state::update(cx, |state| state.rules = rules);
        self.rules_editor = None;
        self.error = None;
        cx.emit(DesktopEvent::Notice(
            "已保存演示规则；点击“演示异动”可验证新阈值".into(),
        ));
        cx.notify();
    }

    fn source(&self, symbol: Symbol, group: Option<&WatchGroup>) -> SourceContext {
        SourceContext {
            date: DATE.into(),
            sector: group.map_or_else(|| "盯盘观察".into(), |group| group.name.clone()),
            symbols: vec![symbol],
            reason: group.map_or_else(|| "来自盯盘异动的观察".into(), |group| group.note.clone()),
            origin: PageId::Watch,
        }
    }

    fn inspect(&mut self, symbol: Symbol, group: Option<WatchGroup>, cx: &mut Context<Self>) {
        self.selected_symbol = Some(symbol);
        if let Some(group) = &group {
            self.selected_group = Some(group.id.clone());
        }
        cx.emit(DesktopEvent::Inspect(self.source(symbol, group.as_ref())));
        cx.notify();
    }

    fn open_alert(&mut self, alert: WatchAlert, cx: &mut Context<Self>) {
        desktop_state::update(cx, |state| {
            if let Some(item) = state.alerts.iter_mut().find(|item| item.id == alert.id) {
                item.read = true;
            }
        });
        let group = cx
            .global::<DesktopState>()
            .watch_groups
            .iter()
            .find(|group| group.name == alert.group_name && group.symbols.contains(&alert.symbol))
            .or_else(|| {
                cx.global::<DesktopState>()
                    .watch_groups
                    .iter()
                    .find(|group| group.symbols.contains(&alert.symbol))
            })
            .cloned();
        self.selected_symbol = Some(alert.symbol);
        self.selected_group = group.as_ref().map(|group| group.id.clone());
        cx.emit(DesktopEvent::Inspect(SourceContext {
            date: DATE.into(),
            sector: alert.group_name,
            symbols: vec![alert.symbol],
            reason: format!("{} · {}", alert.title, alert.detail),
            origin: PageId::Watch,
        }));
        cx.notify();
    }

    fn demo_alert(&mut self, cx: &mut Context<Self>) {
        let state = cx.global::<DesktopState>();
        if !state.monitoring {
            return;
        }
        let rules: Vec<_> = state
            .rules
            .iter()
            .filter(|rule| rule.enabled)
            .cloned()
            .collect();
        if rules.is_empty() {
            cx.emit(DesktopEvent::Notice("请先启用至少一条预警规则".into()));
            return;
        }
        let available: Vec<_> = state
            .watch_groups
            .iter()
            .flat_map(|group| {
                group
                    .symbols
                    .iter()
                    .map(|symbol| (*symbol, group.name.clone()))
            })
            .collect();
        if available.is_empty() {
            cx.emit(DesktopEvent::Notice("请先向盯盘分组添加股票".into()));
            return;
        }
        let count = self.demo_counter;
        self.demo_counter += 1;
        let (symbol, group_name) = available[count % available.len()].clone();
        let rule = rules[count % rules.len()].clone();
        let measured = if rule.id == "near-limit" {
            rule.threshold * 0.48
        } else {
            rule.threshold + 0.32
        };
        let compare = if rule.id == "near-limit" {
            "≤"
        } else {
            "≥"
        };
        let detail = format!(
            "{} {:.2}{} {compare} {:.2}{} · 演示值 #{}",
            rule.label,
            measured,
            rule.unit,
            rule.threshold,
            rule.unit,
            count + 1
        );
        desktop_state::update(cx, |state| {
            let id = state.new_id("alert");
            state.alerts.insert(
                0,
                WatchAlert {
                    id,
                    symbol,
                    rule_id: rule.id,
                    title: rule.title,
                    detail,
                    group_name,
                    time: "14:32:00".into(),
                    read: false,
                },
            );
            state.alerts.truncate(100);
        });
        cx.emit(DesktopEvent::Notice(
            "已生成一条模拟异动，行情价格保持不变".into(),
        ));
        cx.notify();
    }

    fn sidebar(
        &self,
        groups: &[WatchGroup],
        alerts: &[WatchAlert],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let unread = alerts.iter().filter(|alert| !alert.read).count();
        let mut navigation = div().flex().flex_col().gap_1().px_2();
        for (index, group) in groups.iter().enumerate() {
            let id = group.id.clone();
            let selected = self.selected_group.as_ref() == Some(&id);
            navigation = navigation.child(
                div()
                    .id(SharedString::from(format!("watch-nav-{id}")))
                    .test_support()
                    .h(px(40.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded(px(5.))
                    .cursor_pointer()
                    .bg(rgb(if selected { 0xeaf5f4 } else { 0xffffff }))
                    .hover(|style| style.bg(rgb(0xf3f7f8)))
                    .child(div().size(px(7.)).rounded_full().bg(rgb(group.color)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(group.name.clone()),
                    )
                    .child(small(group.symbols.len().to_string()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_group = Some(id.clone());
                        this.scroll.scroll_to_item(1 + index / this.actual_columns);
                        cx.notify();
                    })),
            );
        }
        let mut recent = div().flex().flex_col().px_3();
        for alert in alerts.iter().take(3) {
            let selected = alert.clone();
            recent = recent.child(
                div()
                    .id(SharedString::from(format!("watch-recent-{}", alert.id)))
                    .test_support()
                    .py_3()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .cursor_pointer()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .hover(|style| style.bg(rgb(0xf6f9fa)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().size(px(5.)).rounded_full().bg(rgb(if alert.read {
                                LINE
                            } else {
                                TEAL
                            })))
                            .child(div().flex_1().text_size(px(12.)).child(alert.title.clone()))
                            .child(small(alert.time.clone())),
                    )
                    .child(
                        div()
                            .pl_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(alert.symbol.name())
                            .child(small(alert.symbol.code())),
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.open_alert(selected.clone(), cx)),
                    ),
            );
        }
        if alerts.is_empty() {
            recent = recent.child(
                div()
                    .py_8()
                    .text_center()
                    .child(small("暂无异动，点击演示异动生成")),
            );
        }
        let rule_count = cx
            .global::<DesktopState>()
            .rules
            .iter()
            .filter(|rule| rule.enabled)
            .count();
        panel()
            .w(px(216.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .py_2()
            .gap_2()
            .child(
                div()
                    .h(px(38.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(bold("我的分组"))
                    .child(
                        icon_button("watch-add-group-side", IconName::Plus)
                            .tooltip("新建盯盘分组")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin_group(None, window, cx)
                            })),
                    ),
            )
            .child(navigation)
            .child(div().mx_3().my_3().h(px(1.)).bg(rgb(LINE)))
            .child(
                div()
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(bold("最近异动"))
                    .child(tag(format!("{unread} 未读"), TEAL)),
            )
            .child(recent)
            .child(
                div().px_3().child(
                    button("watch-show-alerts", "查看全部异动")
                        .w_full()
                        .icon(IconName::ChevronRight)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let count = cx
                                .global::<DesktopState>()
                                .watch_groups
                                .len()
                                .div_ceil(this.actual_columns);
                            this.scroll.scroll_to_item(count + 1);
                            cx.notify();
                        })),
                ),
            )
            .child(
                div()
                    .px_3()
                    .pt_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(small(if cx.global::<DesktopState>().monitoring {
                        "模拟规则已就绪"
                    } else {
                        "已暂停生成异动"
                    }))
                    .child(
                        button("watch-rule-settings", format!("{rule_count} 条规则"))
                            .ghost()
                            .icon(IconName::Settings2)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.begin_rules(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn group_card(&self, group: &WatchGroup, cx: &mut Context<Self>) -> AnyElement {
        let mut members = group.symbols.clone();
        if self.sorting != 0 {
            members.sort_by(|left, right| {
                let ordering = market::quote(*left, SNAPSHOT_TICK)
                    .change_pct
                    .total_cmp(&market::quote(*right, SNAPSHOT_TICK).change_pct);
                if self.sorting == 1 {
                    ordering.reverse()
                } else {
                    ordering
                }
            });
        }
        let focused = self.selected_group.as_ref() == Some(&group.id);
        let edit_group = group.clone();
        let add_group = group.clone();
        let mut table = div().flex().flex_col().child(
            div()
                .h(px(30.))
                .px_3()
                .flex()
                .items_center()
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .bg(rgb(0xf8fafb))
                .child(div().flex_1().min_w(px(86.)).child("名称 / 代码"))
                .when(self.actual_columns <= 2, |row| {
                    row.child(div().w(px(66.)).child("走势示意"))
                })
                .child(div().w(px(68.)).text_right().child("最新价"))
                .child(div().w(px(70.)).text_right().child("涨跌幅"))
                .when(self.actual_columns <= 2, |row| {
                    row.child(div().w(px(72.)).text_right().child("月热点*"))
                })
                .child(div().w(px(24.))),
        );
        for symbol in members {
            let quote = market::quote(symbol, SNAPSHOT_TICK);
            let color = if quote.change_pct >= 0. { UP } else { DOWN };
            let item_group = group.clone();
            table = table.child(
                div()
                    .id(SharedString::from(format!(
                        "watch-stock-{}-{}",
                        group.id,
                        symbol.code()
                    )))
                    .test_support()
                    .h(px(49.))
                    .px_3()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(0xeff2f5))
                    .cursor_pointer()
                    .bg(rgb(if focused && self.selected_symbol == Some(symbol) {
                        0xeaf6f5
                    } else {
                        0xffffff
                    }))
                    .hover(|style| style.bg(rgb(0xf3f8f8)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(86.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().truncate().child(symbol.name()))
                            .child(small(format!("{} {}", symbol.code(), symbol.market()))),
                    )
                    .when(self.actual_columns <= 2, |row| {
                        row.child(sparkline(symbol, quote.change_pct >= 0., color))
                    })
                    .child(
                        div()
                            .w(px(68.))
                            .text_right()
                            .text_color(rgb(color))
                            .child(format!("{:.2}", quote.price)),
                    )
                    .child(
                        div()
                            .w(px(70.))
                            .text_right()
                            .text_color(rgb(color))
                            .child(format!("{:+.2}%", quote.change_pct)),
                    )
                    .when(self.actual_columns <= 2, |row| {
                        row.child(
                            div()
                                .w(px(72.))
                                .flex()
                                .flex_col()
                                .items_end()
                                .child(div().text_size(px(12.)).child(sector(symbol)))
                                .child(small(format!("{} 次 · 示例", hotspot_count(symbol)))),
                        )
                    })
                    .child(
                        icon_button(
                            SharedString::from(format!(
                                "watch-market-{}-{}",
                                group.id,
                                symbol.code()
                            )),
                            IconName::ChevronRight,
                        )
                        .w(px(24.))
                        .tooltip("打开行情")
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.stop_propagation();
                            cx.emit(DesktopEvent::OpenMarket(symbol));
                        })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.inspect(symbol, Some(item_group.clone()), cx)
                    })),
            );
        }
        if group.symbols.is_empty() {
            let empty_group = group.clone();
            table = table.child(
                div()
                    .h(px(147.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .child(
                        Icon::new(IconName::Folder)
                            .size(px(24.))
                            .text_color(rgb(MUTED)),
                    )
                    .child(small("这个分组还没有股票"))
                    .child(
                        button(
                            SharedString::from(format!("watch-empty-add-{}", group.id)),
                            "添加股票",
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.begin_group(Some(empty_group.clone()), window, cx)
                            },
                        )),
                    ),
            );
        }
        let selected = self
            .selected_symbol
            .filter(|symbol| focused && group.symbols.contains(symbol));
        let draft_group = group.clone();
        let footer = div()
            .h(px(36.))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .child(small(
                selected.map_or("单击速览 · 右箭头打开行情", Symbol::name),
            ))
            .when_some(selected, |row, symbol| {
                row.child(
                    button(
                        SharedString::from(format!("watch-draft-{}", group.id)),
                        "写预案",
                    )
                    .ghost()
                    .text_color(rgb(TEAL))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.emit(DesktopEvent::CreateDraft(
                            this.source(symbol, Some(&draft_group)),
                        ));
                    })),
                )
            })
            .child(
                button(
                    SharedString::from(format!("watch-group-add-{}", group.id)),
                    "添加股票",
                )
                .ghost()
                .icon(IconName::Plus)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.begin_group(Some(add_group.clone()), window, cx)
                })),
            );
        panel()
            .id(SharedString::from(format!("watch-group-{}", group.id)))
            .test_support()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .border_color(rgb(if focused { 0x8ccac7 } else { LINE }))
            .child(
                div()
                    .h(px(42.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(3.))
                            .h(px(16.))
                            .rounded(px(1.))
                            .bg(rgb(group.color)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(group.name.clone()),
                    )
                    .child(small(group.symbols.len().to_string()))
                    .child(
                        icon_button(
                            SharedString::from(format!("watch-group-edit-{}", group.id)),
                            IconName::Ellipsis,
                        )
                        .tooltip("管理分组")
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.begin_group(Some(edit_group.clone()), window, cx)
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .px_3()
                    .pb_3()
                    .truncate()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(if group.note.is_empty() {
                        "添加观察备注，记录这个分组的关注点".to_owned()
                    } else {
                        group.note.clone()
                    }),
            )
            .child(table)
            .child(footer)
            .into_any_element()
    }

    fn alerts_panel(&self, alerts: &[WatchAlert], cx: &mut Context<Self>) -> AnyElement {
        let unread = alerts.iter().filter(|alert| !alert.read).count();
        let mut body = div().flex().flex_col();
        for alert in alerts
            .iter()
            .filter(|alert| !self.unread_only || !alert.read)
        {
            let item = alert.clone();
            body = body.child(
                div()
                    .id(SharedString::from(format!("watch-alert-{}", alert.id)))
                    .test_support()
                    .h(px(47.))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_3()
                    .border_b_1()
                    .border_color(rgb(0xeff2f5))
                    .cursor_pointer()
                    .bg(rgb(if alert.read { 0xffffff } else { 0xf8fbfb }))
                    .hover(|style| style.bg(rgb(0xeef6f6)))
                    .child(
                        div()
                            .w(px(6.))
                            .h(px(6.))
                            .rounded_full()
                            .bg(rgb(if alert.read { LINE } else { TEAL })),
                    )
                    .child(
                        div()
                            .w(px(70.))
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child(alert.time.clone()),
                    )
                    .child(div().w(px(100.)).child(alert.symbol.name()))
                    .child(div().w(px(90.)).child(tag(alert.title.clone(), TEAL)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .truncate()
                            .child(alert.detail.clone()),
                    )
                    .child(
                        div()
                            .w(px(132.))
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .truncate()
                            .child(alert.group_name.clone()),
                    )
                    .child(
                        Icon::new(IconName::ChevronRight)
                            .size(px(14.))
                            .text_color(rgb(MUTED)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.open_alert(item.clone(), cx))),
            );
        }
        if alerts.is_empty() || (self.unread_only && unread == 0) {
            body = body.child(
                div()
                    .h(px(100.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(small(if self.unread_only {
                        "未读异动已处理完毕"
                    } else {
                        "暂无异动 · 点击“演示异动”查看规则效果"
                    })),
            );
        }
        panel().id("watch-alerts").test_support().overflow_hidden()
            .child(div().h(px(52.)).px_4().flex().items_center().gap_3()
                .child(Icon::new(IconName::Bell).size(px(16.)).text_color(rgb(MUTED)))
                .child(bold("异动记录")).child(tag(format!("{unread} 条未读"), TEAL))
                .child(div().flex_1().child(small("最新在上 · 最多 100 条")))
                .child(button("watch-demo-alert", "演示异动").icon(IconName::Activity).disabled(!cx.global::<DesktopState>().monitoring).on_click(cx.listener(|this, _, _, cx| this.demo_alert(cx))))
                .child(icon_button("watch-unread-only", IconName::ListFilter).tooltip(if self.unread_only { "查看全部异动" } else { "只看未读异动" }).text_color(rgb(if self.unread_only { TEAL } else { MUTED })).on_click(cx.listener(|this, _, _, cx| { this.unread_only = !this.unread_only; cx.notify(); })))
                .child(icon_button("watch-alerts-read", IconName::CheckCheck).tooltip("全部标为已读").disabled(unread == 0).on_click(cx.listener(|_, _, _, cx| desktop_state::update(cx, |state| { for alert in &mut state.alerts { alert.read = true; } }))))
                .child(icon_button("watch-alerts-clear", IconName::Trash).tooltip("清除已读记录").disabled(!alerts.iter().any(|alert| alert.read)).on_click(cx.listener(|_, _, _, cx| desktop_state::update(cx, |state| state.alerts.retain(|alert| !alert.read))))))
            .child(div().h(px(28.)).px_4().flex().items_center().gap_3().bg(rgb(0xf8fafb)).text_size(px(12.)).text_color(rgb(MUTED))
                .child(div().w(px(6.))).child(div().w(px(70.)).child("时间")).child(div().w(px(100.)).child("股票")).child(div().w(px(90.)).child("触发规则")).child(div().flex_1().child("规则详情 / 演示测量值")).child(div().w(px(158.)).child("所属分组")))
            .child(body)
            .child(div().px_4().py_3().child(small("仅展示本地模拟事件；静音开关保存偏好，本阶段不播放音频。月热点为研究样例，非已完成的历史计算。")))
            .into_any_element()
    }

    fn group_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let editor = self.editor.as_ref().expect("group editor");
        let query = self.search.read(cx).value().trim().to_lowercase();
        let mut members = div()
            .id("watch-member-list")
            .max_h(px(245.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .border_1()
            .border_color(rgb(LINE))
            .rounded(px(5.));
        let mut matches = 0;
        for symbol in Symbol::A_SHARES.into_iter().chain(Symbol::LEGACY_DEFAULTS) {
            if !query.is_empty()
                && !symbol.code().to_lowercase().contains(&query)
                && !symbol.name().contains(&query)
            {
                continue;
            }
            matches += 1;
            let checked = editor.symbols.contains(&symbol);
            members = members.child(
                div()
                    .id(SharedString::from(format!(
                        "watch-member-row-{}",
                        symbol.code()
                    )))
                    .test_support()
                    .h(px(38.))
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .cursor_pointer()
                    .border_b_1()
                    .border_color(rgb(0xf0f3f5))
                    .hover(|style| style.bg(rgb(0xf0f8f7)))
                    .child(
                        Checkbox::new(SharedString::from(format!(
                            "watch-member-{}",
                            symbol.code()
                        )))
                        .checked(checked)
                        .accessibility_label(format!("选择 {} {}", symbol.name(), symbol.code()))
                        .on_change(cx.listener(
                            move |this, checked, _, cx| {
                                cx.stop_propagation();
                                if let Some(editor) = &mut this.editor {
                                    editor.symbols.retain(|item| *item != symbol);
                                    if *checked {
                                        editor.symbols.push(symbol);
                                    }
                                }
                                cx.notify();
                            },
                        )),
                    )
                    .child(div().w(px(86.)).child(symbol.name()))
                    .child(small(symbol.code()))
                    .child(div().flex_1())
                    .child(tag(symbol.market(), MUTED))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(editor) = &mut this.editor {
                            if editor.symbols.contains(&symbol) {
                                editor.symbols.retain(|item| *item != symbol);
                            } else {
                                editor.symbols.push(symbol);
                            }
                        }
                        cx.notify();
                    })),
            );
        }
        if matches == 0 {
            members = members.child(
                div()
                    .py_6()
                    .text_center()
                    .child(small("模拟目录中没有匹配股票")),
            );
        }
        let mut colors = div().flex().items_center().gap_3();
        for color in GROUP_COLORS {
            colors = colors.child(
                div()
                    .id(SharedString::from(format!("watch-color-{color}")))
                    .size(px(23.))
                    .rounded_full()
                    .bg(rgb(color))
                    .border_2()
                    .border_color(rgb(if color == editor.color { INK } else { 0xffffff }))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(editor) = &mut this.editor {
                            editor.color = color;
                        }
                        cx.notify();
                    })),
            );
        }
        let mut content = div()
            .flex()
            .flex_col()
            .p_5()
            .gap_4()
            .child(dialog_title(
                if editor.id.is_some() {
                    "管理盯盘分组"
                } else {
                    "新建盯盘分组"
                },
                "每个分组独立管理股票，不会修改行情组件中的股票列表。",
                "watch-group-close",
                cx.listener(|this, _, _, cx| {
                    this.editor = None;
                    this.error = None;
                    cx.notify();
                }),
            ))
            .child(field(
                "分组名称",
                Input::new(&self.name).id("watch-group-name").w_full(),
            ))
            .child(field(
                "观察备注",
                Input::new(&self.note).id("watch-group-note").w_full(),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(small("识别颜色"))
                    .child(colors),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(bold("选择成员"))
                    .child(small(format!("已选择 {} 只", editor.symbols.len()))),
            )
            .child(Input::new(&self.search).id("watch-group-search").w_full())
            .child(members);
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(UP))
                    .child(error.clone()),
            );
        }
        if editor.deleting {
            content = content.child(
                div()
                    .p_3()
                    .bg(rgb(0xfff4f4))
                    .border_1()
                    .border_color(rgb(0xf2d6dd))
                    .rounded(px(5.))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_color(rgb(UP))
                            .child("删除这个分组及其成员设置？历史异动记录会保留。"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                button("watch-delete-confirm", "确认删除分组")
                                    .text_color(rgb(UP))
                                    .on_click(cx.listener(|this, _, _, cx| this.delete_group(cx))),
                            )
                            .child(button("watch-delete-cancel", "保留分组").on_click(
                                cx.listener(|this, _, _, cx| {
                                    if let Some(editor) = &mut this.editor {
                                        editor.deleting = false;
                                    }
                                    cx.notify();
                                }),
                            )),
                    ),
            );
        } else {
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(editor.id.is_some(), |row| {
                        row.child(
                            button("watch-group-delete", "删除分组")
                                .ghost()
                                .text_color(rgb(UP))
                                .icon(IconName::Trash)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Some(editor) = &mut this.editor {
                                        editor.deleting = true;
                                    }
                                    cx.notify();
                                })),
                        )
                    })
                    .child(div().flex_1())
                    .child(button("watch-group-cancel", "取消").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.editor = None;
                            this.error = None;
                            cx.notify();
                        },
                    )))
                    .child(
                        button("watch-group-save", "保存分组")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.save_group(cx))),
                    ),
            );
        }
        overlay(
            panel()
                .id("watch-group-dialog")
                .w(px(570.))
                .max_h(relative(0.94))
                .overflow_y_scroll()
                .child(content),
        )
        .into_any_element()
    }

    fn rules_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let rules = self.rules_editor.as_ref().expect("rules editor");
        let mut content = div().p_5().flex().flex_col().gap_5().child(dialog_title(
            "预警规则",
            "本地演示规则，仅在点击“演示异动”时生成事件。",
            "watch-rules-close",
            cx.listener(|this, _, _, cx| {
                this.rules_editor = None;
                this.error = None;
                cx.notify();
            }),
        ));
        for (index, rule) in rules.iter().enumerate() {
            content = content.child(
                div()
                    .border_1()
                    .border_color(rgb(LINE))
                    .rounded(px(6.))
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(bold(rule.title.clone()))
                            .child(
                                button(
                                    SharedString::from(format!("watch-rule-enable-{}", rule.id)),
                                    if rule.enabled {
                                        "已启用"
                                    } else {
                                        "已停用"
                                    },
                                )
                                .ghost()
                                .icon(if rule.enabled {
                                    IconName::Check
                                } else {
                                    IconName::Minus
                                })
                                .text_color(rgb(if rule.enabled { TEAL } else { MUTED }))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if let Some(rules) = &mut this.rules_editor {
                                            rules[index].enabled = !rules[index].enabled;
                                        }
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(12.))
                                    .text_color(rgb(MUTED))
                                    .child(rule.label.clone()),
                            )
                            .child(
                                Input::new(&self.rule_inputs[index])
                                    .id(SharedString::from(format!("watch-rule-{}", rule.id)))
                                    .w(px(86.)),
                            )
                            .child(small(rule.unit.clone())),
                    ),
            );
        }
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(UP))
                    .child(error.clone()),
            );
        }
        content = content
            .child(small(
                "规则状态与阈值随工作区保存。暂停监控不会清空已有异动。",
            ))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(button("watch-rules-cancel", "取消").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.rules_editor = None;
                            this.error = None;
                            cx.notify();
                        },
                    )))
                    .child(
                        button("watch-rules-save", "保存规则")
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.save_rules(cx))),
                    ),
            );
        overlay(
            panel()
                .id("watch-rules-dialog")
                .w(px(530.))
                .max_h(relative(0.94))
                .overflow_y_scroll()
                .child(content),
        )
        .into_any_element()
    }
}

impl Render for WatchPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.global::<DesktopState>();
        let groups = state.watch_groups.clone();
        let alerts = state.alerts.clone();
        let monitoring = state.monitoring;
        let muted = state.muted;
        let unique = groups
            .iter()
            .flat_map(|group| &group.symbols)
            .collect::<HashSet<_>>()
            .len();
        let width: f32 = window.viewport_size().width.into();
        let available = width - 352.;
        self.actual_columns = if self.columns == 3 && available >= 1_030. {
            3
        } else if self.columns >= 2 && available >= 840. {
            2
        } else {
            1
        };
        let mut layout_buttons = div()
            .flex()
            .items_center()
            .gap_1()
            .p_1()
            .bg(rgb(0xeef2f5))
            .rounded(px(5.));
        for columns in 1..=3 {
            layout_buttons = layout_buttons.child(
                button(
                    SharedString::from(format!("watch-columns-{columns}")),
                    format!("{columns} 列"),
                )
                .ghost()
                .bg(rgb(if self.columns == columns {
                    0xffffff
                } else {
                    0xeef2f5
                }))
                .text_color(rgb(if self.columns == columns { TEAL } else { MUTED }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.columns = columns;
                    cx.notify();
                })),
            );
        }
        let mut main = div()
            .id("watch-main-scroll")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .h_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .h(px(40.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(bold("分组看板"))
                    .child(small(format!("{} 组 / {unique} 只股票", groups.len())))
                    .child(div().flex_1())
                    .child(
                        button(
                            "watch-sort",
                            match self.sorting {
                                1 => "涨幅降序",
                                2 => "涨幅升序",
                                _ => "手动顺序",
                            },
                        )
                        .icon(IconName::ArrowUpDown)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sorting = (this.sorting + 1) % 3;
                            cx.notify();
                        })),
                    )
                    .child(layout_buttons)
                    .child(
                        button("watch-add-group", "新建分组")
                            .icon(IconName::Plus)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin_group(None, window, cx)
                            })),
                    ),
            );
        for row in groups.chunks(self.actual_columns) {
            let mut cards = div().flex_shrink_0().flex().items_start().gap_4();
            for group in row {
                cards = cards.child(self.group_card(group, cx));
            }
            for _ in row.len()..self.actual_columns {
                cards = cards.child(div().flex_1());
            }
            main = main.child(cards);
        }
        if groups.is_empty() {
            main = main.child(
                panel()
                    .h(px(260.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .child(
                        Icon::new(IconName::Folder)
                            .size(px(32.))
                            .text_color(rgb(TEAL)),
                    )
                    .child(bold("按自己的观察思路建立分组"))
                    .child(small("例如情绪核心、趋势跟踪、轮动候选。"))
                    .child(
                        button("watch-add-first", "创建第一个分组")
                            .primary()
                            .icon(IconName::Plus)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.begin_group(None, window, cx)
                            })),
                    ),
            );
        }
        main = main
            .child(div().flex_shrink_0().child(self.alerts_panel(&alerts, cx)))
            .child(div().h(px(12.)).flex_shrink_0());
        let mut page = div()
            .id("watch-page")
            .test_support()
            .relative()
            .size_full()
            .min_w_0()
            .min_h_0()
            .bg(rgb(BG))
            .text_color(rgb(INK))
            .text_size(px(14.))
            .p_5()
            .flex()
            .flex_col()
            .gap_5()
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(rgb(MUTED))
                                    .child("MARKET MONITOR   盘中观察"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .text_size(px(24.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("盯盘"),
                                    )
                                    .child(tag(
                                        if monitoring {
                                            "模拟监控中"
                                        } else {
                                            "监控已暂停"
                                        },
                                        if monitoring { TEAL } else { MUTED },
                                    )),
                            )
                            .child(small("把关注点放在一起，让异动主动来到你面前。")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(small(format!("{DATE}  14:32 快照")))
                            .child(
                                button(
                                    "watch-muted",
                                    if muted {
                                        "已静音"
                                    } else {
                                        "提示音偏好已开"
                                    },
                                )
                                .icon(if muted {
                                    IconName::VolumeX
                                } else {
                                    IconName::Volume2
                                })
                                .tooltip("仅保存静音偏好，本阶段不播放音频")
                                .on_click(cx.listener(
                                    |_, _, _, cx| {
                                        desktop_state::update(cx, |state| {
                                            state.muted = !state.muted
                                        })
                                    },
                                )),
                            )
                            .child(
                                button(
                                    "watch-monitor",
                                    if monitoring {
                                        "暂停监控"
                                    } else {
                                        "继续监控"
                                    },
                                )
                                .icon(if monitoring {
                                    IconName::Pause
                                } else {
                                    IconName::Play
                                })
                                .on_click(cx.listener(
                                    |_, _, _, cx| {
                                        desktop_state::update(cx, |state| {
                                            state.monitoring = !state.monitoring
                                        })
                                    },
                                )),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .items_start()
                    .gap_4()
                    .child(self.sidebar(&groups, &alerts, cx))
                    .child(main),
            );
        if self.editor.is_some() {
            page = page.child(self.group_editor(cx));
        }
        if self.rules_editor.is_some() {
            page = page.child(self.rules_editor(cx));
        }
        page
    }
}

fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id).label(label).small().text_size(px(12.))
}
fn icon_button(id: impl Into<ElementId>, icon: IconName) -> Button {
    Button::new(id)
        .icon(icon)
        .small()
        .ghost()
        .text_color(rgb(MUTED))
}
fn panel() -> Div {
    div()
        .bg(rgb(0xffffff))
        .border_1()
        .border_color(rgb(LINE))
        .rounded(px(7.))
}
fn small(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(12.))
        .text_color(rgb(MUTED))
        .child(text.into())
}
fn bold(text: impl Into<SharedString>) -> Div {
    div().font_weight(FontWeight::SEMIBOLD).child(text.into())
}
fn tag(text: impl Into<SharedString>, color: u32) -> Div {
    div()
        .px_2()
        .py(px(3.))
        .rounded(px(4.))
        .bg(rgb(if color == TEAL { 0xeaf5f3 } else { 0xf0f3f6 }))
        .text_size(px(11.))
        .text_color(rgb(color))
        .child(text.into())
}
fn field(label: &'static str, input: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(small(label))
        .child(input)
}
fn overlay(content: impl IntoElement) -> Stateful<Div> {
    div()
        .id("watch-dialog-overlay")
        .absolute()
        .inset_0()
        .bg(rgba(0x172c454d))
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .child(content)
}
fn dialog_title(
    title: &'static str,
    subtitle: &'static str,
    close_id: &'static str,
    listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .flex()
        .items_start()
        .gap_3()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(small(subtitle)),
        )
        .child(icon_button(close_id, IconName::X).on_click(listener))
}
fn sector(symbol: Symbol) -> &'static str {
    match symbol.code() {
        "603598" => "传媒",
        "001318" => "食品",
        "300750" => "新能源",
        "600519" | "000858" => "白酒",
        "601127" | "002594" => "汽车",
        "300059" => "金融科技",
        "688981" => "半导体",
        "000333" => "家电",
        "002475" => "电子",
        "600036" => "银行",
        _ => "观察",
    }
}
fn hotspot_count(symbol: Symbol) -> usize {
    match symbol.code() {
        "603598" => 8,
        "001318" => 4,
        "300750" | "000858" => 3,
        "601127" | "002475" => 7,
        "002594" => 5,
        "300059" => 6,
        "688981" => 9,
        _ => 2,
    }
}
fn sparkline(symbol: Symbol, rising: bool, color: u32) -> AnyElement {
    let phase = symbol.code().bytes().map(usize::from).sum::<usize>() as f32;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let mut path = PathBuilder::stroke(px(1.3));
            for index in 0..25 {
                let x = index as f32 / 24.;
                let trend = if rising {
                    0.74 - x * 0.50
                } else {
                    0.26 + x * 0.46
                };
                let wave =
                    (index as f32 * 1.65 + phase).sin() * 0.09 + (index as f32 * 0.57).sin() * 0.06;
                let y = (trend + wave).clamp(0.08, 0.92);
                let point = point(
                    bounds.left() + bounds.size.width * x,
                    bounds.top() + bounds.size.height * y,
                );
                if index == 0 {
                    path.move_to(point);
                } else {
                    path.line_to(point);
                }
            }
            if let Ok(path) = path.build() {
                window.paint_path(path, rgb(color));
            }
        },
    )
    .w(px(66.))
    .h(px(30.))
    .into_any_element()
}
