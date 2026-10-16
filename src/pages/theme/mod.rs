//! Theme evidence view over explicit simulated research objects.
use gpui_kit::{
    component::{
        button::ButtonVariants,
        input::{Input, InputEvent, InputState},
    },
    prelude::*,
    *,
};

use crate::app::state::{DesktopEvent, PageId, SourceContext};
use crate::app::ui::{BG, LINE, MUTED, TEAL, heading, muted, page, tag};
use crate::market::Symbol;

use self::fixtures::{ThemeObject, actual_date, theme_objects, valid_date};
use super::research::{
    DATES, EVIDENCE_DATES, SECTORS, button, line, panel, section_title, text_cell,
};

mod fixtures;
mod render;

pub(crate) struct ThemePage {
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
    pub(crate) fn is_editing(&self) -> bool {
        false
    }

    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
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

    pub(crate) fn receive(
        &mut self,
        context: SourceContext,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
}
