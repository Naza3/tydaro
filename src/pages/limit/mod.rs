//! Editable imported limit-up research records.
use gpui_kit::{
    component::{
        Disableable,
        button::ButtonVariants,
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        menu::{DropdownMenu, PopupMenuItem},
    },
    prelude::*,
    *,
};
use tydaro_core::research::ResearchRow;

use crate::app::state::{
    self as desktop_state, Command, DesktopEvent, DesktopState, PageId, SourceContext,
};
use crate::app::ui::{BG, LINE, MUTED, TEAL, heading, muted, page, tag};
use crate::market::Symbol;

use self::query::{Field, Filter, matches, restore_rows, source};
use super::research::{
    DATES, SECTORS, button, panel, sample_rows, section_title, sector_color, sector_reason,
    seed_rows, text_cell,
};

mod editor;
mod query;
mod render;
#[cfg(feature = "test-support")]
mod test_support;
#[cfg(feature = "test-support")]
pub(crate) use test_support::verify_rejected_edits;

enum LimitEdit {
    Import,
    Delete(String),
    Row(String),
}

pub(crate) struct LimitPage {
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
    pub(crate) fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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
            cx.subscribe(&edit_name, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.refresh_pending_input(cx);
                }
                cx.notify();
            }),
            cx.subscribe(&edit_board, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.refresh_pending_input(cx);
                }
                cx.notify();
            }),
            cx.subscribe(&edit_reason, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.refresh_pending_input(cx);
                }
                cx.notify();
            }),
        ];
        let (rows, load_error) = match cx.global::<DesktopState>().research_document() {
            Ok(None) => (seed_rows(), None),
            Ok(Some(document)) => (document.rows.clone(), None),
            Err(error) => (
                Vec::new(),
                Some(format!(
                    "研究资料格式无法读取，原值已保留；本页暂停修改：{error}"
                )),
            ),
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
}
