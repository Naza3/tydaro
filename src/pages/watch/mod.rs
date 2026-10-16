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

use crate::app::state::{
    self as desktop_state, Command, CommandResult, DesktopEvent, DesktopState, PageId,
    SourceContext, WatchAlert, WatchGroup, WatchRule,
};
use crate::app::ui::{BG, DOWN, INK, LINE, MUTED, TEAL, UP};
use crate::market::{self, Symbol};

const SNAPSHOT_TICK: u64 = 272;
const DATE: &str = "2026-10-14";
const GROUP_COLORS: [u32; 6] = [0x009b9a, 0x6686c0, 0xba9466, 0x9b83ad, 0xc27a87, 0x718e62];

struct GroupEditor {
    id: Option<String>,
    symbols: Vec<Symbol>,
    color: u32,
    deleting: bool,
}

pub(crate) struct WatchPage {
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
    rule_subscriptions: Vec<Subscription>,
    error: Option<String>,
    demo_counter: usize,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DesktopEvent> for WatchPage {}

mod editor;
mod render;
mod sidebar;

#[cfg(feature = "test-support")]
mod test_support;
#[cfg(feature = "test-support")]
pub(crate) use test_support::verify_rejected_edits;

impl WatchPage {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("例如：科技 · 趋势跟踪"));
        let note = cx.new(|cx| InputState::new(window, cx).placeholder("记录这个分组的关注点"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索名称或代码"));
        let mut subscriptions = Vec::new();
        for input in [&name, &note, &search] {
            subscriptions.push(cx.subscribe(input, |this, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    this.error = None;
                    this.refresh_pending_input(cx);
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
            rule_subscriptions: Vec::new(),
            error: None,
            demo_counter: 0,
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn is_editing(&self) -> bool {
        self.editor.is_some() || self.rules_editor.is_some()
    }

    fn simple_command(&mut self, command: Command, cx: &mut Context<Self>) {
        if let Err(error) = desktop_state::dispatch(cx, command) {
            cx.emit(DesktopEvent::Notice(format!("操作未应用：{error}")));
            cx.notify();
        }
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
        if let Err(error) = desktop_state::dispatch(cx, Command::ReadAlert(alert.id.clone())) {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
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
        if let Err(error) = desktop_state::dispatch(
            cx,
            Command::RecordDemoAlert(WatchAlert {
                id: String::new(),
                symbol,
                rule_id: rule.id,
                title: rule.title,
                detail,
                group_name,
                time: "14:32:00".into(),
                read: false,
            }),
        ) {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        cx.emit(DesktopEvent::Notice(
            "已生成一条模拟异动，行情价格保持不变".into(),
        ));
        cx.notify();
    }
}

fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    tydaro_ui::action::action(id, label, tydaro_ui::action::ActionSize::Small)
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
