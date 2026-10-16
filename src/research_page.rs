//! Native research views. Fixtures demonstrate provenance and navigation;
//! editing imported rows never silently recalculates the independent theme examples.
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        menu::{DropdownMenu, PopupMenuItem},
    },
    prelude::*,
    *,
};
use serde::{Deserialize, Serialize};

use crate::{
    desktop_state::{self, DesktopEvent, DesktopState, PageId, SourceContext},
    desktop_ui::{BG, LINE, MUTED, TEAL, heading, muted, page, panel as base_panel, tag},
    market::Symbol,
};

const DATES: [&str; 6] = [
    "2026-10-14",
    "2026-10-13",
    "2026-10-12",
    "2026-10-09",
    "2026-10-08",
    "2026-10-07",
];
const EVIDENCE_DATES: [&str; 10] = [
    "2026-10-14",
    "2026-10-13",
    "2026-10-12",
    "2026-10-09",
    "2026-10-08",
    "2026-10-07",
    "2026-09-30",
    "2026-09-29",
    "2026-09-28",
    "2026-09-25",
];
const SECTORS: [&str; 4] = ["AI 应用", "汽车产业链", "消费复苏", "科技硬件"];
const ORDERS: [[usize; 4]; 6] = [
    [0, 1, 2, 3],
    [1, 0, 3, 2],
    [2, 3, 0, 1],
    [0, 3, 1, 2],
    [3, 2, 0, 1],
    [1, 2, 3, 0],
];

#[derive(Clone, Serialize, Deserialize, Debug)]
struct ResearchRow {
    id: String,
    date: String,
    sector: String,
    symbol: Symbol,
    name: String,
    board: String,
    reason: String,
    source: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct ResearchSaved {
    rows: Vec<ResearchRow>,
}

#[derive(Clone, Copy, PartialEq)]
enum Field {
    All,
    Sector,
    Code,
    Name,
    Board,
    Reason,
    Date,
    Source,
}
impl Field {
    const ALL: [Self; 8] = [
        Self::All,
        Self::Sector,
        Self::Code,
        Self::Name,
        Self::Board,
        Self::Reason,
        Self::Date,
        Self::Source,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::All => "全部字段",
            Self::Sector => "板块",
            Self::Code => "股票代码",
            Self::Name => "股票名称",
            Self::Board => "连板字段",
            Self::Reason => "关键词",
            Self::Date => "交易日",
            Self::Source => "来源文件",
        }
    }
    fn value(self, row: &ResearchRow) -> String {
        match self {
            Self::All => format!(
                "{} {} {} {} {} {} {}",
                row.date,
                row.sector,
                row.symbol.code(),
                row.name,
                row.board,
                row.reason,
                row.source
            ),
            Self::Sector => row.sector.clone(),
            Self::Code => row.symbol.code().to_owned(),
            Self::Name => row.name.clone(),
            Self::Board => row.board.clone(),
            Self::Reason => row.reason.clone(),
            Self::Date => row.date.clone(),
            Self::Source => row.source.clone(),
        }
    }
}
#[derive(Clone)]
struct Filter {
    field: Field,
    value: String,
}

fn sector_symbols(sector: usize) -> Vec<Symbol> {
    match sector {
        0 => vec![Symbol::Yinli, Symbol::Eastmoney, Symbol::Luxshare],
        1 => vec![Symbol::Seres, Symbol::Byd, Symbol::Catl],
        2 => vec![
            Symbol::Yangguang,
            Symbol::Moutai,
            Symbol::Wuliangye,
            Symbol::Midea,
        ],
        _ => vec![Symbol::Smic, Symbol::Luxshare, Symbol::Midea],
    }
}
fn reason(symbol: Symbol) -> &'static str {
    match symbol {
        Symbol::Yinli => "AI 营销 + 数字传媒 + 内容生产",
        Symbol::Eastmoney => "互联网金融 + AI 应用",
        Symbol::Luxshare => "消费电子 + AI 终端 + 产业链",
        Symbol::Seres => "新能源汽车 + 智能驾驶",
        Symbol::Byd => "新能源汽车 + 海外布局",
        Symbol::Catl => "动力电池 + 储能",
        Symbol::Yangguang => "乳业 + 消费复苏 + 区域品牌",
        Symbol::Moutai | Symbol::Wuliangye => "白酒 + 消费复苏",
        Symbol::Midea => "家用电器 + 智能制造",
        Symbol::Smic => "半导体 + 国产替代",
        _ => "示例研究资料",
    }
}
fn sector_reason(sector: &str) -> &'static str {
    match sector {
        "AI 应用" => "内容生产与营销应用受到关注，观察核心标的与板块跟随的持续性。",
        "汽车产业链" => "新车型与产业链需求预期共振，关注整车及电池方向的承接。",
        "消费复苏" => "消费预期延续，乳业、白酒及家电呈现轮动，关注量能变化。",
        _ => "国产替代与终端创新预期带动关注，观察半导体与消费电子联动。",
    }
}
fn sector_color(sector: &str) -> u32 {
    match sector {
        "AI 应用" => TEAL,
        "汽车产业链" => 0x6686c0,
        "消费复苏" => 0xb58a50,
        _ => 0x987cac,
    }
}
fn sample_rows(date: &str, day: usize) -> Vec<ResearchRow> {
    ORDERS[day % ORDERS.len()]
        .iter()
        .enumerate()
        .flat_map(|(position, sector)| {
            let count = if (day + position) % 3 == 1 { 2 } else { 3 };
            sector_symbols(*sector)
                .into_iter()
                .take(count)
                .enumerate()
                .map(move |(index, symbol)| ResearchRow {
                    id: format!("{date}-{}-{}", SECTORS[*sector], symbol.code()),
                    date: date.into(),
                    sector: SECTORS[*sector].into(),
                    symbol,
                    name: symbol.name().into(),
                    board: if index == 0 {
                        ["4天4板", "3天3板", "2天2板", "首板"][day % 4]
                    } else if index == 1 {
                        "2天2板"
                    } else {
                        "首板"
                    }
                    .into(),
                    reason: reason(symbol).into(),
                    source: format!("{date}_涨停复盘_示例.csv"),
                })
        })
        .collect()
}
fn seed_rows() -> Vec<ResearchRow> {
    DATES
        .iter()
        .enumerate()
        .flat_map(|(day, date)| sample_rows(date, day))
        .collect()
}
fn restore_rows(rows: &mut Vec<ResearchRow>, restored: Vec<ResearchRow>) {
    // Reimporting a deleted day before undo must not duplicate IDs or replace
    // records subsequently edited by the user.
    for row in restored {
        if !rows.iter().any(|existing| existing.id == row.id) {
            rows.push(row);
        }
    }
    rows.sort_by(|a, b| b.date.cmp(&a.date));
}
fn matches(row: &ResearchRow, filters: &[Filter]) -> bool {
    filters.iter().all(|filter| {
        filter
            .field
            .value(row)
            .to_lowercase()
            .contains(&filter.value.to_lowercase())
    })
}
fn source(rows: &[ResearchRow], origin: PageId) -> SourceContext {
    let mut symbols = Vec::new();
    for row in rows {
        if !symbols.contains(&row.symbol) {
            symbols.push(row.symbol);
        }
    }
    SourceContext {
        date: rows.first().map_or(DATES[0], |r| &r.date).into(),
        sector: rows.first().map_or("研究观察", |r| &r.sector).into(),
        symbols,
        reason: rows.first().map_or("", |r| &r.reason).into(),
        origin,
    }
}
fn button(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Button {
    Button::new(id)
        .label(label)
        .small()
        .flex_shrink_0()
        .text_size(px(12.))
}
fn panel() -> Div {
    // Pages scroll as a whole. Intrinsic cards must retain their content height
    // instead of shrinking and clipping inputs or the last imported-day row.
    base_panel().flex_shrink_0()
}
fn text_cell(text: impl Into<SharedString>, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .overflow_hidden()
        .child(text.into())
}
fn line() -> Div {
    div().h(px(1.)).w_full().bg(rgb(LINE))
}
fn section_title(title: impl Into<SharedString>) -> Div {
    div()
        .h(px(46.))
        .px_4()
        .flex()
        .items_center()
        .gap_2()
        .border_b_1()
        .border_color(rgb(LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title.into())
}

enum LimitEdit {
    Import,
    Delete(String),
    Row(String),
}

pub struct LimitPage {
    rows: Vec<ResearchRow>,
    selected_date: String,
    selected_sector: String,
    selected_code: Symbol,
    search_tab: bool,
    collapsed: bool,
    field: Field,
    filters: Vec<Filter>,
    query: Entity<InputState>,
    editing: Option<LimitEdit>,
    edit_name: Entity<InputState>,
    edit_board: Entity<InputState>,
    edit_reason: Entity<TextareaState>,
    deleted: Vec<ResearchRow>,
    error: Option<String>,
    load_error: Option<String>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<DesktopEvent> for LimitPage {}

impl LimitPage {
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query =
            cx.new(|cx| InputState::new(window, cx).placeholder("输入关键词，按 Enter 添加条件"));
        let edit_name = cx.new(|cx| InputState::new(window, cx).placeholder("股票名称"));
        let edit_board = cx.new(|cx| InputState::new(window, cx).placeholder("例如 4天4板"));
        let edit_reason = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 5)
                .placeholder("资料中的涨停原因")
        });
        let subscriptions = vec![
            cx.subscribe(&query, |this, _, event, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    // The native input only emits PressEnter outside IME composition.
                    this.add_filter(cx);
                }
                cx.notify();
            }),
            cx.subscribe(&edit_name, |_, _, _: &InputEvent, cx| cx.notify()),
            cx.subscribe(&edit_board, |_, _, _: &InputEvent, cx| cx.notify()),
        ];
        let saved = cx.global::<DesktopState>().research.clone();
        let (rows, load_error) = if saved.is_null() {
            (seed_rows(), None)
        } else {
            match serde_json::from_value::<ResearchSaved>(saved) {
                Ok(saved)
                    if saved.rows.len() <= 100_000
                        && saved.rows.iter().all(|row| {
                            valid_date(&row.date)
                                && !row.id.is_empty()
                                && !row.sector.trim().is_empty()
                        }) =>
                {
                    (saved.rows, None)
                }
                _ => (
                    Vec::new(),
                    Some(
                        "研究资料格式无法读取，原值已保留；本页暂停修改，请检查本地工作台文档。"
                            .to_owned(),
                    ),
                ),
            }
        };
        let selected_date = rows
            .first()
            .map_or_else(|| DATES[0].into(), |r| r.date.clone());
        let selected_sector = rows
            .first()
            .map_or_else(|| SECTORS[0].into(), |r| r.sector.clone());
        Self {
            rows,
            selected_date,
            selected_sector,
            selected_code: Symbol::Yinli,
            search_tab: false,
            collapsed: false,
            field: Field::All,
            filters: Vec::new(),
            query,
            editing: None,
            edit_name,
            edit_board,
            edit_reason,
            deleted: Vec::new(),
            error: None,
            load_error,
            _subscriptions: subscriptions,
        }
    }
    fn persist(&self, cx: &mut Context<Self>) {
        if self.load_error.is_some() {
            return;
        }
        if let Ok(value) = serde_json::to_value(ResearchSaved {
            rows: self.rows.clone(),
        }) {
            desktop_state::update(cx, |state| state.research = value);
        }
    }
    fn dates(&self) -> Vec<String> {
        let mut dates: Vec<_> = self.rows.iter().map(|r| r.date.clone()).collect();
        dates.sort();
        dates.dedup();
        dates.reverse();
        dates
    }
    fn selected(&self) -> Vec<ResearchRow> {
        self.rows
            .iter()
            .filter(|r| r.date == self.selected_date && r.sector == self.selected_sector)
            .cloned()
            .collect()
    }
    fn visible(&self) -> Vec<ResearchRow> {
        if self.search_tab {
            self.rows
                .iter()
                .filter(|r| matches(r, &self.filters))
                .cloned()
                .collect()
        } else {
            self.selected()
        }
    }
    fn choose(&mut self, date: String, sector: String, cx: &mut Context<Self>) {
        self.selected_date = date;
        self.selected_sector = sector;
        self.search_tab = false;
        if let Some(row) = self.selected().first() {
            self.selected_code = row.symbol;
        }
        cx.notify();
    }
    fn add_filter(&mut self, cx: &mut Context<Self>) {
        let value = self.query.read(cx).value().trim().to_owned();
        if value.is_empty() {
            return;
        }
        if !self
            .filters
            .iter()
            .any(|f| f.field == self.field && f.value == value)
        {
            self.filters.push(Filter {
                field: self.field,
                value,
            });
        }
        self.search_tab = true;
        cx.notify();
    }
    fn begin_edit(&mut self, row: ResearchRow, window: &mut Window, cx: &mut Context<Self>) {
        if self.load_error.is_some() {
            return;
        }
        if self.editing.is_some() {
            cx.emit(DesktopEvent::Notice("请先保存或取消当前资料编辑".into()));
            return;
        }
        self.edit_name
            .update(cx, |input, cx| input.set_value(row.name, window, cx));
        self.edit_board
            .update(cx, |input, cx| input.set_value(row.board, window, cx));
        self.edit_reason
            .update(cx, |input, cx| input.set_value(row.reason, window, cx));
        self.editing = Some(LimitEdit::Row(row.id));
        self.error = None;
        self.edit_name
            .update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }
    fn confirm_edit(&mut self, cx: &mut Context<Self>) {
        if self.load_error.is_some() {
            return;
        }
        match self.editing.as_ref() {
            Some(LimitEdit::Import) => {
                if !self.rows.iter().any(|r| r.date == "2026-10-15") {
                    let mut rows = sample_rows("2026-10-15", 0);
                    rows.append(&mut self.rows);
                    self.rows = rows;
                    self.selected_date = "2026-10-15".into();
                    self.selected_sector = SECTORS[0].into();
                    self.search_tab = false;
                }
            }
            Some(LimitEdit::Delete(date)) => {
                self.deleted = self
                    .rows
                    .iter()
                    .filter(|r| &r.date == date)
                    .cloned()
                    .collect();
                self.rows.retain(|r| &r.date != date);
                if let Some(row) = self.rows.first() {
                    self.selected_date = row.date.clone();
                    self.selected_sector = row.sector.clone();
                }
            }
            Some(LimitEdit::Row(id)) => {
                let name = self.edit_name.read(cx).value().trim().to_owned();
                let board = self.edit_board.read(cx).value().trim().to_owned();
                if name.is_empty() || board.is_empty() {
                    self.error = Some("请填写名称和连板字段；空白原因可以保留。".into());
                    cx.notify();
                    return;
                }
                let reason = self.edit_reason.read(cx).value().to_string();
                if let Some(row) = self.rows.iter_mut().find(|r| &r.id == id) {
                    row.name = name;
                    row.board = board;
                    row.reason = reason;
                }
            }
            None => return,
        }
        self.persist(cx);
        self.editing = None;
        self.error = None;
        cx.emit(DesktopEvent::Notice(
            "涨停资料已修改；题材页独立示例不会自动重算".into(),
        ));
        cx.notify();
    }
    fn editor(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(edit) = &self.editing else {
            return div().into_any_element();
        };
        let (title, description) = match edit {
            LimitEdit::Import => (
                "导入示例资料",
                if self.rows.iter().any(|row| row.date == "2026-10-15") {
                    "2026-10-15 的示例资料已导入，无需重复添加。关闭此面板可继续查看或编辑。"
                        .to_owned()
                } else {
                    "载入 2026-10-15 的内置样例，用于体验资料流程。当前不读取真实文件，不执行旧格式兼容或算法重算。".to_owned()
                },
            ),
            LimitEdit::Delete(date) => (
                "删除整日资料",
                format!(
                    "将删除 {date} 的 {} 条资料。删除后可在本页撤销最近一次整日删除。",
                    self.rows.iter().filter(|r| &r.date == date).count()
                ),
            ),
            LimitEdit::Row(_) => (
                "编辑资料",
                "只修改此条导入记录，不修改股票目录或历史题材样例。".to_owned(),
            ),
        };
        panel()
            .id("limit-editor")
            .border_color(rgb(TEAL))
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(muted(description))
            .when(matches!(edit, LimitEdit::Row(_)), |view| {
                view.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .child(muted("股票名称"))
                                .child(Input::new(&self.edit_name).id("limit-edit-name")),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(muted("连板字段"))
                                .child(Input::new(&self.edit_board).id("limit-edit-board")),
                        ),
                )
                .child(
                    div()
                        .id("limit-edit-reason")
                        .test_support()
                        .child(Textarea::new(&self.edit_reason).h(px(96.))),
                )
            })
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_color(rgb(0xc92f57)).child(error))
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(button("limit-edit-cancel", "取消").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.editing = None;
                            this.error = None;
                            cx.notify();
                        },
                    )))
                    .child(
                        button("limit-edit-confirm", "确认")
                            .disabled(
                                matches!(edit, LimitEdit::Import)
                                    && self.rows.iter().any(|row| row.date == "2026-10-15"),
                            )
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| this.confirm_edit(cx))),
                    ),
            )
            .into_any_element()
    }
    fn matrix(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut matrix = panel().child(
            section_title("多日板块概览")
                .child(tag(format!("最近 {} 个导入日", self.dates().len()), MUTED))
                .child(div().flex_1())
                .child(
                    button(
                        "limit-collapse",
                        if self.collapsed { "展开" } else { "收起" },
                    )
                    .ghost()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.collapsed = !this.collapsed;
                        cx.notify();
                    })),
                ),
        );
        if !self.collapsed {
            matrix = matrix.child(
                div()
                    .px_4()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child("每日按原资料顺序排列，列不代表同名板块；数字为资料条数。"),
            );
            for date in self.dates() {
                let day: Vec<_> = self.rows.iter().filter(|r| r.date == date).collect();
                let mut sectors = Vec::new();
                for row in &day {
                    if !sectors.contains(&row.sector) {
                        sectors.push(row.sector.clone());
                    }
                }
                let mut row = div()
                    .h(px(44.))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(
                        div()
                            .w(px(76.))
                            .flex_shrink_0()
                            .font_weight(FontWeight::MEDIUM)
                            .child(date[5..].replace('-', ".")),
                    );
                for (index, sector) in sectors.iter().enumerate() {
                    let items: Vec<_> = day.iter().filter(|r| &r.sector == sector).collect();
                    let selected = date == self.selected_date && sector == &self.selected_sector;
                    let matched =
                        self.filters.is_empty() || items.iter().any(|r| matches(r, &self.filters));
                    let next_date = date.clone();
                    let next_sector = sector.clone();
                    row = row.child(
                        div()
                            .id(SharedString::from(format!("limit-sector-{date}-{index}")))
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .h(px(30.))
                            .px_3()
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(rgb(if selected { TEAL } else { LINE }))
                            .bg(rgb(if selected { 0xe4f5f2 } else { 0xf7f9fa }))
                            .text_color(rgb(if matched { sector_color(sector) } else { MUTED }))
                            .cursor_pointer()
                            .child(div().truncate().child(sector.clone()))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(items.len().to_string()),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.choose(next_date.clone(), next_sector.clone(), cx)
                            })),
                    );
                }
                matrix = matrix.child(
                    row.child(
                        div()
                            .w(px(32.))
                            .text_right()
                            .text_color(rgb(MUTED))
                            .child(day.len().to_string()),
                    ),
                );
            }
            matrix = matrix.child(
                div()
                    .px_4()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .bg(rgb(0xf8fafb))
                    .child("✓ 原始顺序已保留    ·    示例资料不代表实际涨停结果"),
            );
        }
        matrix.into_any_element()
    }
    fn details(&self, cx: &mut Context<Self>) -> AnyElement {
        let visible = self.visible();
        let selected = self.selected();
        let context = source(&selected, PageId::Limit);
        let context_theme = context.clone();
        let context_watch = context.clone();
        let mut details = panel().child(
            section_title("")
                .child(
                    button("limit-details-tab", "板块明细")
                        .when(!self.search_tab, |b| b.primary())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.search_tab = false;
                            cx.notify();
                        })),
                )
                .child(
                    button("limit-results-tab", "搜索结果")
                        .when(self.search_tab, |b| b.primary())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.search_tab = true;
                            cx.notify();
                        })),
                )
                .child(tag(format!("{} 条", visible.len()), MUTED))
                .child(div().flex_1())
                .child(
                    button("limit-copy", "复制代码")
                        .disabled(visible.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            let context = source(&this.visible(), PageId::Limit);
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                context
                                    .symbols
                                    .iter()
                                    .map(|s| s.code())
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ));
                            cx.emit(DesktopEvent::Notice(format!(
                                "已复制 {} 个股票代码",
                                context.symbols.len()
                            )));
                        })),
                )
                .child(
                    button("limit-delete-day", "删除整日")
                        .disabled(selected.is_empty() || self.editing.is_some())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.editing = Some(LimitEdit::Delete(this.selected_date.clone()));
                            cx.notify();
                        })),
                ),
        );
        if !self.search_tab {
            details =
                details.child(
                    div()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(19.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(self.selected_sector.clone()),
                                )
                                .child(tag(self.selected_date.clone(), MUTED))
                                .child(div().flex_1())
                                .child(button("limit-open-theme", "研究题材 →").on_click(
                                    cx.listener(move |_, _, _, cx| {
                                        cx.emit(DesktopEvent::OpenTheme(context_theme.clone()))
                                    }),
                                ))
                                .child(
                                    button("limit-add-watch", "+ 加入盯盘")
                                        .disabled(selected.is_empty())
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            cx.emit(DesktopEvent::AddToWatch(context_watch.clone()))
                                        })),
                                ),
                        )
                        .child(muted(sector_reason(&self.selected_sector))),
                );
        }
        details = details.child(
            div()
                .h(px(32.))
                .px_4()
                .flex()
                .items_center()
                .bg(rgb(BG))
                .text_size(px(12.))
                .text_color(rgb(MUTED))
                .child(text_cell("股票 / 代码", 142.))
                .child(text_cell("日期 / 连板", 110.))
                .child(div().flex_1().child("涨停原因 / 来源"))
                .child(text_cell("操作", 196.)),
        );
        for row in visible {
            let inspect = source(std::slice::from_ref(&row), PageId::Limit);
            let draft = inspect.clone();
            let edit_row = row.clone();
            let locate = row.clone();
            details = details.child(
                div()
                    .id(SharedString::from(format!("limit-row-{}", row.id)))
                    .min_h(px(66.))
                    .px_4()
                    .py_2()
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .bg(rgb(
                        if self.selected_code == row.symbol && !self.search_tab {
                            0xf1f9f8
                        } else {
                            0xffffff
                        },
                    ))
                    .child(
                        div()
                            .w(px(142.))
                            .flex_shrink_0()
                            .child(
                                button(
                                    SharedString::from(format!("limit-inspect-{}", row.id)),
                                    row.name.clone(),
                                )
                                .ghost()
                                .on_click(cx.listener(
                                    move |_, _, _, cx| {
                                        cx.emit(DesktopEvent::Inspect(inspect.clone()))
                                    },
                                )),
                            )
                            .child(muted(row.symbol.code())),
                    )
                    .child(
                        div()
                            .w(px(110.))
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .child(row.date.clone())
                            .child(tag(row.board.clone(), TEAL)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .pr_3()
                            .text_size(px(13.))
                            .child(row.reason.clone())
                            .child(muted(row.source.clone())),
                    )
                    .child(
                        div()
                            .w(px(196.))
                            .flex_shrink_0()
                            .flex()
                            .gap_1()
                            .child(
                                button(
                                    SharedString::from(format!("limit-edit-{}", row.id)),
                                    "编辑",
                                )
                                .ghost()
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.begin_edit(edit_row.clone(), window, cx)
                                    },
                                )),
                            )
                            .when(self.search_tab, |view| {
                                view.child(
                                    button(
                                        SharedString::from(format!("limit-locate-{}", row.id)),
                                        "定位",
                                    )
                                    .ghost()
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.choose(
                                                locate.date.clone(),
                                                locate.sector.clone(),
                                                cx,
                                            );
                                            this.selected_code = locate.symbol;
                                        },
                                    )),
                                )
                            })
                            .child(
                                button(
                                    SharedString::from(format!("limit-draft-{}", row.id)),
                                    "引用预案",
                                )
                                .ghost()
                                .on_click(cx.listener(
                                    move |_, _, _, cx| {
                                        cx.emit(DesktopEvent::CreateDraft(draft.clone()))
                                    },
                                )),
                            ),
                    ),
            );
        }
        if self.visible().is_empty() {
            details = details.child(
                div()
                    .p_8()
                    .text_color(rgb(MUTED))
                    .child("没有匹配资料。移除条件或选择其他日期与板块。"),
            );
        }
        details.into_any_element()
    }
}

impl Render for LimitPage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.weak_entity();
        let result_count = self
            .rows
            .iter()
            .filter(|row| matches(row, &self.filters))
            .count();
        page()
            .id("limit-page")
            .test_support()
            .overflow_y_scroll()
            .gap_3()
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(heading(
                        "整理资料 · 发现线索",
                        "涨停资料",
                        "按导入日追溯板块与个股，保留每一条来源",
                    ))
                    .child(
                        button("limit-import", "导入示例资料")
                            .disabled(self.load_error.is_some() || self.editing.is_some())
                            .primary()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.editing = Some(LimitEdit::Import);
                                cx.notify();
                            })),
                    ),
            )
            .when_some(self.load_error.clone(), |view, message| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .text_color(rgb(0xc92f57))
                        .child(message),
                )
            })
            .child(
                panel()
                    .p_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_color(rgb(MUTED)).child("筛选资料"))
                    .child(
                        button("limit-filter-field", self.field.label()).dropdown_menu(
                            move |mut menu, _, _| {
                                for field in Field::ALL {
                                    let owner = owner.clone();
                                    menu = menu.item(PopupMenuItem::new(field.label()).on_click(
                                        move |_, _, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.field = field;
                                                cx.notify();
                                            });
                                        },
                                    ));
                                }
                                menu
                            },
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Input::new(&self.query).id("limit-filter")),
                    )
                    .child(
                        button("limit-filter-add", "+ 添加条件")
                            .disabled(self.query.read(cx).value().trim().is_empty())
                            .on_click(cx.listener(|this, _, _, cx| this.add_filter(cx))),
                    )
                    .child(muted("AND · 同时满足")),
            )
            .when(!self.filters.is_empty(), |view| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .children(self.filters.iter().enumerate().map(|(index, filter)| {
                            button(
                                SharedString::from(format!("limit-filter-remove-{index}")),
                                format!("{}：{} ×", filter.field.label(), filter.value),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.filters.remove(index);
                                    cx.notify();
                                },
                            ))
                        }))
                        .child(button("limit-filter-clear", "清空条件").ghost().on_click(
                            cx.listener(|this, _, _, cx| {
                                this.filters.clear();
                                this.search_tab = false;
                                cx.notify();
                            }),
                        ))
                        .child(
                            muted(format!("{result_count} 条命中"))
                                .id("limit-result-count")
                                .aria_label(format!("{result_count} 条命中"))
                                .test_support(),
                        ),
                )
            })
            .when(self.editing.is_some(), |view| view.child(self.editor(cx)))
            .when(!self.deleted.is_empty(), |view| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(muted(format!("已删除 {} 条资料", self.deleted.len())))
                        .child(
                            button("limit-undo-delete", "撤销整日删除").on_click(cx.listener(
                                |this, _, _, cx| {
                                    let restored = std::mem::take(&mut this.deleted);
                                    if let Some(row) = restored.first() {
                                        this.selected_date = row.date.clone();
                                        this.selected_sector = row.sector.clone();
                                    }
                                    restore_rows(&mut this.rows, restored);
                                    this.persist(cx);
                                    cx.notify();
                                },
                            )),
                        ),
                )
            })
            .child(self.matrix(cx))
            .child(self.details(cx))
            .child(
                muted(
                    "资料修改保存在本地工作台文档中。题材页是独立研究样例，不随这些资料自动重算。",
                )
                .flex_shrink_0(),
            )
    }
}

#[derive(Clone)]
struct ThemeObject {
    symbol: Symbol,
    trigger: &'static str,
    sector: &'static str,
    secondary: &'static str,
    method: &'static str,
    keyword: &'static str,
    related: Vec<Symbol>,
    generic: Vec<Symbol>,
    counts: [u32; 4],
    first: [u32; 4],
    second: [u32; 4],
}
fn theme_objects() -> Vec<ThemeObject> {
    vec![
        ThemeObject {
            symbol: Symbol::Yinli,
            trigger: "2026-10-14",
            sector: SECTORS[0],
            secondary: "文化传媒",
            method: "连续识别 + 字段识别",
            keyword: "AI 营销",
            related: vec![Symbol::Eastmoney, Symbol::Luxshare],
            generic: vec![Symbol::Smic],
            counts: [4, 6, 8, 9],
            first: [3, 4, 5, 5],
            second: [1, 1, 2, 2],
        },
        ThemeObject {
            symbol: Symbol::Yangguang,
            trigger: "2026-10-13",
            sector: SECTORS[2],
            secondary: "食品饮料",
            method: "字段识别",
            keyword: "消费复苏",
            related: vec![Symbol::Moutai, Symbol::Wuliangye, Symbol::Midea],
            generic: vec![Symbol::Cmb],
            counts: [3, 5, 4, 7],
            first: [2, 3, 2, 4],
            second: [1, 1, 1, 2],
        },
        ThemeObject {
            symbol: Symbol::Seres,
            trigger: "2026-10-12",
            sector: SECTORS[1],
            secondary: "智能驾驶",
            method: "连续识别",
            keyword: "新能源汽车",
            related: vec![Symbol::Byd, Symbol::Catl],
            generic: vec![Symbol::Luxshare],
            counts: [5, 6, 5, 8],
            first: [4, 3, 3, 5],
            second: [1, 2, 1, 2],
        },
        ThemeObject {
            symbol: Symbol::Smic,
            trigger: "2026-10-09",
            sector: SECTORS[3],
            secondary: "国产替代",
            method: "连续识别 + 字段识别",
            keyword: "国产替代",
            related: vec![Symbol::Luxshare, Symbol::Midea],
            generic: vec![Symbol::Eastmoney],
            counts: [3, 4, 6, 6],
            first: [2, 2, 4, 3],
            second: [1, 1, 1, 2],
        },
    ]
}
fn valid_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(i, b)| i != 4 && i != 7 && !b.is_ascii_digit())
    {
        return false;
    }
    let year = date[..4].parse::<u32>().unwrap_or(0);
    let month = date[5..7].parse::<usize>().unwrap_or(0);
    let day = date[8..].parse::<u32>().unwrap_or(0);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (1..=12).contains(&month) && day > 0 && day <= days[month - 1]
}
fn actual_date(requested: &str) -> Option<&'static str> {
    if !valid_date(requested) {
        return None;
    }
    EVIDENCE_DATES.into_iter().find(|date| *date <= requested)
}

pub struct ThemePage {
    selected: Symbol,
    from: Entity<InputState>,
    requested: Entity<InputState>,
    query: Entity<InputState>,
    keyword_tab: bool,
    related_tab: bool,
    specific_only: bool,
    incoming: Option<SourceContext>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<DesktopEvent> for ThemePage {}
impl ThemePage {
    pub fn is_editing(&self) -> bool {
        false
    }
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let from = cx.new(|cx| InputState::new(window, cx).default_value("2026-09-01"));
        let requested = cx.new(|cx| InputState::new(window, cx).default_value(DATES[0]));
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("股票、代码或题材"));
        let subscriptions = [&from, &requested, &query]
            .into_iter()
            .map(|input| cx.subscribe(input, |_, _, _: &InputEvent, cx| cx.notify()))
            .collect();
        Self {
            selected: Symbol::Yinli,
            from,
            requested,
            query,
            keyword_tab: false,
            related_tab: false,
            specific_only: false,
            incoming: None,
            _subscriptions: subscriptions,
        }
    }
    pub fn receive(&mut self, context: SourceContext, window: &mut Window, cx: &mut Context<Self>) {
        let object = theme_objects()
            .into_iter()
            .find(|o| context.symbols.contains(&o.symbol))
            .or_else(|| {
                theme_objects()
                    .into_iter()
                    .find(|o| o.sector == context.sector)
            });
        if let Some(object) = object {
            self.selected = object.symbol;
            self.from
                .update(cx, |input, cx| input.set_value("2026-09-01", window, cx));
            // Keep the exact source request visible, even if this fixture's trigger is later.
            self.requested.update(cx, |input, cx| {
                input.set_value(context.date.clone(), window, cx)
            });
            self.query
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.keyword_tab = false;
            self.related_tab = false;
        }
        self.incoming = Some(context);
        cx.notify();
    }
    fn objects(&self, cx: &App) -> Vec<ThemeObject> {
        let from = self.from.read(cx).value();
        let requested = self.requested.read(cx).value();
        let query = self.query.read(cx).value().trim().to_lowercase();
        if !valid_date(&from) || !valid_date(&requested) || from > requested {
            return Vec::new();
        }
        theme_objects()
            .into_iter()
            .filter(|o| {
                o.trigger >= from.as_str()
                    && o.trigger <= requested.as_str()
                    && format!(
                        "{} {} {} {}",
                        o.symbol.code(),
                        o.symbol.name(),
                        o.sector,
                        o.secondary
                    )
                    .to_lowercase()
                    .contains(&query)
            })
            .collect()
    }
    fn context(&self, object: &ThemeObject, symbols: Vec<Symbol>, cx: &App) -> SourceContext {
        let requested = self.requested.read(cx).value();
        SourceContext {
            date: actual_date(&requested).unwrap_or(&requested).into(),
            sector: object.sector.into(),
            symbols,
            reason: format!(
                "{}；四板触发日 {}，来源：示例涨停资料。",
                object.keyword, object.trigger
            ),
            origin: PageId::Theme,
        }
    }
    fn evidence(&self, object: &ThemeObject, cx: &mut Context<Self>) -> AnyElement {
        let mut dates: Vec<_> = EVIDENCE_DATES
            .into_iter()
            .filter(|d| *d <= object.trigger)
            .take(4)
            .collect();
        dates.reverse();
        let symbol = object.symbol;
        let watch = self.context(object, vec![symbol], cx);
        let draft = watch.clone();
        let mut body = div().flex_1().min_w_0().flex().flex_col().gap_3().child(
            panel()
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .size(px(36.))
                                .rounded(px(8.))
                                .bg(rgb(0xe2f4f0))
                                .text_color(rgb(TEAL))
                                .flex()
                                .items_center()
                                .justify_center()
                                .font_weight(FontWeight::BOLD)
                                .child(
                                    object
                                        .symbol
                                        .name()
                                        .chars()
                                        .next()
                                        .unwrap_or('研')
                                        .to_string(),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .child(
                                    div()
                                        .text_size(px(22.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(object.symbol.name()),
                                )
                                .child(muted(object.symbol.code())),
                        )
                        .child(
                            button("theme-open-market", "查看行情 ↗").on_click(cx.listener(
                                move |_, _, _, cx| cx.emit(DesktopEvent::OpenMarket(symbol)),
                            )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(tag(object.sector, TEAL))
                        .child(tag(object.method, MUTED))
                        .child(div().flex_1())
                        .child(
                            button("theme-add-watch", "+ 加入盯盘").on_click(cx.listener(
                                move |_, _, _, cx| cx.emit(DesktopEvent::AddToWatch(watch.clone())),
                            )),
                        )
                        .child(
                            button("theme-create-draft", "引用为预案 →")
                                .primary()
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(DesktopEvent::CreateDraft(draft.clone()))
                                })),
                        ),
                )
                .child(line())
                .child(div().flex().items_center().justify_between().children(
                    dates.iter().enumerate().map(|(index, date)| {
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(tag(format!("{} 板", index + 1), TEAL))
                            .child(muted(date[5..].replace('-', ".")))
                    }),
                ))
                .child(muted("历史归属固定于四板周期；四日指已导入日。")),
        );
        let mut evidence = panel().child(
            section_title("")
                .child(
                    button("theme-cycle-tab", "周期证据")
                        .when(!self.keyword_tab, |b| b.primary())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.keyword_tab = false;
                            cx.notify();
                        })),
                )
                .child(
                    button("theme-keywords-tab", "八日关键词")
                        .when(self.keyword_tab, |b| b.primary())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.keyword_tab = true;
                            cx.notify();
                        })),
                ),
        );
        if self.keyword_tab {
            evidence = evidence.child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("关键词：{}", object.keyword)),
                    )
                    .child(muted(
                        "向前八个已导入日的命中样例，模糊命中不是确定业务关联。",
                    )),
            );
            let peers: Vec<_> = object
                .related
                .iter()
                .chain(&object.generic)
                .copied()
                .collect();
            for (index, date) in EVIDENCE_DATES
                .into_iter()
                .filter(|d| *d <= object.trigger)
                .take(8)
                .enumerate()
            {
                let peer = peers[index % peers.len()];
                let mut context = self.context(object, vec![peer], cx);
                context.date = date.into();
                evidence = evidence.child(
                    div()
                        .min_h(px(43.))
                        .px_4()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_t_1()
                        .border_color(rgb(LINE))
                        .child(muted(date[5..].replace('-', ".")))
                        .child(
                            button(
                                SharedString::from(format!("theme-keyword-{index}")),
                                peer.name(),
                            )
                            .ghost()
                            .on_click(cx.listener(
                                move |_, _, _, cx| cx.emit(DesktopEvent::Inspect(context.clone())),
                            )),
                        )
                        .child(div().flex_1().text_size(px(12.)).child(if index % 3 == 0 {
                            object.keyword.to_owned()
                        } else {
                            format!("{}…", object.keyword.chars().take(2).collect::<String>())
                        }))
                        .child(tag(
                            if index % 3 == 0 { "完全" } else { "模糊" },
                            if index % 3 == 0 { TEAL } else { 0xb58a50 },
                        )),
                );
            }
        } else {
            let metrics = [
                ("涨停累计", object.counts.iter().sum::<u32>()),
                ("首板累计", object.first.iter().sum::<u32>()),
                ("二板累计", object.second.iter().sum::<u32>()),
            ];
            evidence = evidence
                .child(
                    div()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("板块梯队"))
                        .child(muted("四个已导入日累计次数，同股跨日分别计数"))
                        .child(div().flex().gap_3().children(metrics.into_iter().map(
                            |(label, count)| {
                                div()
                                    .flex_1()
                                    .p_3()
                                    .rounded(px(6.))
                                    .bg(rgb(BG))
                                    .child(muted(label))
                                    .child(
                                        div()
                                            .text_size(px(25.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(format!("{count} 次")),
                                    )
                            },
                        )))
                        .child(
                            div()
                                .h(px(146.))
                                .flex()
                                .items_end()
                                .gap_4()
                                .px_4()
                                .children(dates.iter().enumerate().map(|(index, date)| {
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(rgb(TEAL))
                                                .child(object.counts[index].to_string()),
                                        )
                                        .child(
                                            div()
                                                .w(px(42.))
                                                .h(px(object.counts[index] as f32 * 9.))
                                                .rounded_t(px(4.))
                                                .bg(rgb(0x69b9b0))
                                                .flex()
                                                .items_end()
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .h(px(object.second[index] as f32 * 9.))
                                                        .bg(rgb(TEAL)),
                                                ),
                                        )
                                        .child(muted(date[5..].replace('-', ".")))
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("周期内题材归属"),
                                )
                                .child(
                                    button(
                                        "theme-specific-only",
                                        if self.specific_only {
                                            "✓ 仅具体题材"
                                        } else {
                                            "仅具体题材"
                                        },
                                    )
                                    .ghost()
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.specific_only = !this.specific_only;
                                            cx.notify();
                                        },
                                    )),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .px_4()
                        .py_2()
                        .bg(rgb(BG))
                        .text_size(px(12.))
                        .text_color(rgb(MUTED))
                        .child(div().flex_1().child("归属题材"))
                        .child(text_cell("出现次数", 90.))
                        .child(text_cell("日期覆盖", 90.)),
                )
                .children(
                    [
                        (object.sector, "4 次", "全部 4 日"),
                        (object.secondary, "3 次", "第 2—4 日"),
                        ("其他", "1 次", "第 1 日"),
                    ]
                    .into_iter()
                    .filter(|(name, _, _)| !self.specific_only || *name != "其他")
                    .map(|(name, count, coverage)| {
                        div()
                            .px_4()
                            .h(px(38.))
                            .flex()
                            .items_center()
                            .border_t_1()
                            .border_color(rgb(LINE))
                            .child(div().flex_1().child(name))
                            .child(text_cell(count, 90.))
                            .child(text_cell(coverage, 90.))
                    }),
                );
        }
        body = body.child(evidence).child(muted(
            "引用研究依据会创建独立草稿，已有预案和人工热点保持原样。",
        ));
        body.into_any_element()
    }
    fn related(&self, object: &ThemeObject, cx: &mut Context<Self>) -> AnyElement {
        let requested = self.requested.read(cx).value();
        let actual = actual_date(&requested);
        let mut view = panel()
            .flex()
            .flex_col()
            .child(section_title("截止日联动").child(tag(actual.unwrap_or("无数据"), MUTED)))
            .child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(muted("历史归属 × 截止日资料"))
                    .child(
                        div()
                            .text_size(px(13.))
                            .child("同题材与关键词提供两条观察线索。"),
                    ),
            );
        for (label, generic, peers) in [
            ("同板块", false, &object.related),
            ("泛板块关键词", true, &object.generic),
        ] {
            view = view.child(
                div()
                    .px_4()
                    .py_2()
                    .bg(rgb(BG))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("{label}   {} 只", peers.len())),
            );
            view = view.child(div().px_4().py_2().child(muted(if generic {
                "仅匹配“公告 / 其他”等泛板块资料"
            } else {
                "四板周期具体题材在截止日的其他股票"
            })));
            for (index, peer) in peers.iter().enumerate() {
                let inspect = self.context(object, vec![*peer], cx);
                let watch = inspect.clone();
                view = view.child(
                    div()
                        .p_4()
                        .border_t_1()
                        .border_color(rgb(LINE))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    button(
                                        SharedString::from(format!("theme-peer-{}", peer.code())),
                                        peer.name(),
                                    )
                                    .ghost()
                                    .on_click(cx.listener(
                                        move |_, _, _, cx| {
                                            cx.emit(DesktopEvent::Inspect(inspect.clone()))
                                        },
                                    )),
                                )
                                .child(div().flex_1())
                                .child(tag(
                                    if generic {
                                        "模糊"
                                    } else if index == 0 {
                                        "2天2板"
                                    } else {
                                        "首板"
                                    },
                                    if generic { 0xb58a50 } else { TEAL },
                                )),
                        )
                        .child(muted(format!(
                            "{}  ·  {}",
                            peer.code(),
                            if generic {
                                object.keyword
                            } else {
                                object.sector
                            }
                        )))
                        .child(
                            div().flex().justify_end().child(
                                button(
                                    SharedString::from(format!("theme-peer-watch-{}", peer.code())),
                                    "+ 盯盘",
                                )
                                .ghost()
                                .on_click(cx.listener(
                                    move |_, _, _, cx| {
                                        cx.emit(DesktopEvent::AddToWatch(watch.clone()))
                                    },
                                )),
                            ),
                        ),
                );
            }
        }
        let mut symbols = object.related.clone();
        for symbol in &object.generic {
            if !symbols.contains(symbol) {
                symbols.push(*symbol);
            }
        }
        let watch = self.context(object, symbols.clone(), cx);
        view.child(
            div()
                .p_3()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    button("theme-copy-related", "复制代码").on_click(cx.listener(
                        move |_, _, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                symbols
                                    .iter()
                                    .map(|s| s.code())
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                            ));
                            cx.emit(DesktopEvent::Notice("已复制合并去重后的联动代码".into()));
                        },
                    )),
                )
                .child(
                    button("theme-watch-related", "全部加入盯盘")
                        .primary()
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(DesktopEvent::AddToWatch(watch.clone()))
                        })),
                ),
        )
        .into_any_element()
    }
}

impl Render for ThemePage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let narrow = window.viewport_size().width < px(1400.);
        let requested = self.requested.read(cx).value().to_string();
        let from = self.from.read(cx).value().to_string();
        let actual = actual_date(&requested);
        let objects = self.objects(cx);
        let object = objects
            .iter()
            .find(|o| o.symbol == self.selected)
            .or_else(|| objects.first());
        let mut object_list = panel()
            .w(px(226.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .child(section_title("四板观察对象").child(tag(objects.len().to_string(), MUTED)))
            .child(
                div()
                    .p_3()
                    .child(Input::new(&self.query).id("theme-search")),
            );
        for item in &objects {
            let symbol = item.symbol;
            let active = object.is_some_and(|o| o.symbol == symbol);
            object_list = object_list.child(
                div()
                    .id(SharedString::from(format!(
                        "theme-object-{}",
                        symbol.code()
                    )))
                    .test_support()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .border_l(px(3.))
                    .border_color(rgb(if active { TEAL } else { LINE }))
                    .bg(rgb(if active { 0xeaf7f4 } else { 0xffffff }))
                    .cursor_pointer()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(symbol.name()))
                            .child(tag("4 板", TEAL)),
                    )
                    .child(muted(format!(
                        "{}   {} 触发",
                        symbol.code(),
                        &item.trigger[5..]
                    )))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(TEAL))
                            .child(format!("{}  ›", item.sector)),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = symbol;
                        this.related_tab = false;
                        cx.notify();
                    })),
            );
        }
        object_list = object_list.child(
            div()
                .p_4()
                .child(muted("按已导入日计算。漏导资料可能影响周期识别。")),
        );
        let mut content = div()
            .flex_shrink_0()
            .flex()
            .items_start()
            .gap_3()
            .child(object_list);
        if let Some(object) = object {
            if narrow {
                content = content.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    button("theme-evidence-view", "周期与证据")
                                        .when(!self.related_tab, |b| b.primary())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.related_tab = false;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    button("theme-related-view", "截止日联动")
                                        .when(self.related_tab, |b| b.primary())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.related_tab = true;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(if self.related_tab {
                            self.related(object, cx)
                        } else {
                            self.evidence(object, cx)
                        }),
                );
            } else {
                content = content.child(self.evidence(object, cx)).child(
                    div()
                        .w(px(286.))
                        .flex_shrink_0()
                        .child(self.related(object, cx)),
                );
            }
        } else {
            content = content.child(
                panel()
                    .flex_1()
                    .p_8()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_size(px(20.)).child("没有符合条件的四板周期"))
                    .child(muted(
                        "扩大日期范围或清除搜索。早期资料不会被替换成未来的四板对象。",
                    ))
                    .child(
                        button("theme-reset-empty", "重置筛选").on_click(cx.listener(
                            |this, _, window, cx| {
                                this.from
                                    .update(cx, |i, cx| i.set_value("2026-09-01", window, cx));
                                this.requested
                                    .update(cx, |i, cx| i.set_value(DATES[0], window, cx));
                                this.query.update(cx, |i, cx| i.set_value("", window, cx));
                                this.incoming = None;
                                cx.notify();
                            },
                        )),
                    ),
            );
        }
        page().id("theme-page").test_support().overflow_y_scroll().gap_3()
            .child(heading("追溯周期 · 理解关联", "题材研究", "对象、证据与截止日联动，放在同一条研究路径上").flex_shrink_0())
            .child(panel().p_3().flex().flex_wrap().items_center().gap_2()
                .child(muted("触发区间"))
                .child(div().w(px(135.)).child(Input::new(&self.from).id("theme-from-date")))
                .child(muted("至"))
                .child(div().w(px(135.)).child(Input::new(&self.requested).id("theme-cutoff-date")))
                .child(tag(format!("实际截止日  {}", actual.unwrap_or("无可用资料")), if actual == Some(requested.as_str()) { TEAL } else { 0xb58a50 }))
                .when(actual.is_some() && actual != Some(requested.as_str()), |view| view.child(muted("已回退至最近导入日")))
                .child(div().flex_1())
                .child(button("theme-reset-date", "重置日期").ghost().on_click(cx.listener(|this, _, window, cx| {
                    this.from.update(cx, |i, cx| i.set_value("2026-09-01", window, cx)); this.requested.update(cx, |i, cx| i.set_value(DATES[0], window, cx)); this.incoming = None; cx.notify();
                }))))
            .when(!valid_date(&from) || !valid_date(&requested) || from > requested, |view| view.child(div().flex_shrink_0().text_color(rgb(0xc92f57)).child("请输入有效日期 YYYY-MM-DD，起始日不能晚于截止日。")))
            .when_some(self.incoming.as_ref(), |view, incoming| view.child(div().flex_shrink_0().px_3().py_2().rounded(px(4.)).bg(rgb(0xeaf7f4)).text_size(px(12.)).text_color(rgb(TEAL)).child(format!("来自涨停资料：{} · {} · {}", incoming.date, incoming.sector, incoming.reason))))
            .child(content)
            .child(muted("四板对象、梯队与关键词是固定研究样例；日期用于筛选和解释，不代表已实现旧系统计算算法。").flex_shrink_0())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DATES, Field, Filter, actual_date, matches, restore_rows, sample_rows, seed_rows, source,
        valid_date,
    };
    use crate::{desktop_state::PageId, market::Symbol};
    #[test]
    fn source_order_is_independent_for_each_day_and_zeroes_survive() {
        let first = sample_rows(DATES[0], 0);
        let second = sample_rows(DATES[1], 1);
        assert_eq!(first[0].sector, "AI 应用");
        assert_eq!(second[0].sector, "汽车产业链");
        assert!(first.iter().any(|r| r.symbol.code() == "001318"));
    }
    #[test]
    fn filter_intersection_is_not_union_and_removal_restores_rows() {
        let rows = seed_rows();
        let filters = vec![
            Filter {
                field: Field::Sector,
                value: "AI".into(),
            },
            Filter {
                field: Field::Date,
                value: "2026-10-14".into(),
            },
        ];
        let result: Vec<_> = rows.iter().filter(|r| matches(r, &filters)).collect();
        assert_eq!(result.len(), 3);
        assert!(
            result
                .iter()
                .all(|r| r.date == DATES[0] && r.sector == "AI 应用")
        );
        assert!(rows.iter().filter(|r| matches(r, &filters[..1])).count() > result.len());
    }
    #[test]
    fn cutoff_never_uses_future_data_and_calendar_input_is_validated() {
        assert_eq!(actual_date("2026-10-11"), Some("2026-10-09"));
        assert_eq!(actual_date("2026-09-01"), None);
        assert_eq!(actual_date("2026-02-30"), None);
        assert!(valid_date("2024-02-29"));
        assert!(!valid_date("2026-02-29"));
    }
    #[test]
    fn handoff_retains_exact_date_reason_and_deduplicates_symbols() {
        let rows = sample_rows("2026-10-08", 0);
        let mut selected = vec![rows[0].clone(), rows[0].clone()];
        selected[0].reason = "历史依据".into();
        let context = source(&selected, PageId::Limit);
        assert_eq!(context.date, "2026-10-08");
        assert_eq!(context.reason, "历史依据");
        assert_eq!(context.symbols, vec![Symbol::Yinli]);
    }
    #[test]
    fn undo_after_reimport_preserves_current_edits_without_duplicate_ids() {
        let deleted = sample_rows("2026-10-15", 0);
        let mut current = deleted.clone();
        current[0].reason = "重新导入后的人工修订".into();
        let count = current.len();
        restore_rows(&mut current, deleted);
        assert_eq!(current.len(), count);
        assert_eq!(current[0].reason, "重新导入后的人工修订");
    }
}
