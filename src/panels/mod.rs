//! Business panels share a deterministic quote stream and a local demo ledger.
use crate::market::{self, Period, Symbol};
use crate::model::PanelKind;
use crate::stock_import::{self, ImportError, ImportPreview};
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
    },
    prelude::*,
    *,
};

use self::{
    broker::{DemoBroker, OrderStatus},
    chart::{
        ChartEvent, MarketChart,
        history::{self, HistoryWindow},
    },
    elements::*,
    snapshot::{ContentSnapshot, OrderFilterSnapshot, Side},
    stock_list::StockListPage,
};

mod broker;
pub(crate) mod chart;
mod chart_panel;
mod elements;
mod market_orders;
mod news;
mod order_book;
mod quote;
pub(crate) mod snapshot;
mod stock_list;
mod trades;
mod trading;

pub(crate) use broker::init_broker;
#[cfg(feature = "test-support")]
mod test_support;
#[cfg(feature = "test-support")]
pub(crate) use test_support::{smoke_demo_orders, smoke_market_panels};

const INK: u32 = 0x233348;
const MUTED: u32 = 0x586879;
const LINE: u32 = 0xe8ecf0;
const TEAL: u32 = 0x009b9a;
const DOWN: u32 = 0x007d78;
const RED: u32 = 0xc92f57;

#[derive(Clone, Debug)]
pub(crate) enum ContentEvent {
    SymbolSelected(Symbol),
    PeriodChanged(Period),
    OrderSubmitted,
    ListChanged,
}

/// A panel owns only the native controls needed by its kind. These entities
/// remain window-local and are rebuilt from the durable snapshot on migration.
enum PanelView {
    Watchlist {
        query: Entity<InputState>,
        catalog_query: Entity<InputState>,
        import_text: Entity<TextareaState>,
    },
    Chart(Entity<MarketChart>),
    OrderEntry {
        price: Entity<InputState>,
        quantity: Entity<InputState>,
    },
    OrderDetails {
        min_amount: Entity<InputState>,
        max_amount: Entity<InputState>,
        min_quantity: Entity<InputState>,
        max_quantity: Entity<InputState>,
    },
    ReadOnly,
}

impl PanelView {
    fn inputs(&self) -> Vec<&Entity<InputState>> {
        match self {
            Self::Watchlist {
                query,
                catalog_query,
                ..
            } => vec![query, catalog_query],
            Self::OrderEntry { price, quantity } => vec![price, quantity],
            Self::OrderDetails {
                min_amount,
                max_amount,
                min_quantity,
                max_quantity,
            } => vec![min_amount, max_amount, min_quantity, max_quantity],
            Self::Chart(_) | Self::ReadOnly => Vec::new(),
        }
    }
}

pub(crate) struct PanelContent {
    view: PanelView,
    legacy_content: ContentSnapshot,
    kind: PanelKind,
    symbol: Symbol,
    period: Period,
    tick: u64,
    history: Option<HistoryWindow>,
    stock_list_page: StockListPage,
    import_preview: Result<ImportPreview, ImportError>,
    import_message: Option<(String, bool)>,
    import_loading: bool,
    import_revision: u64,
    import_request: u64,
    members: Vec<Symbol>,
    side: Side,
    sort: u8,
    filter_enabled: bool,
    filter_direction: u8,
    large_trades_only: bool,
    book_queue: bool,
    feedback: Option<(String, bool)>,
    expanded_news: Option<usize>,
    pending_price: Option<f64>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<ContentEvent> for PanelContent {}

impl PanelContent {
    pub(crate) fn new(
        kind: PanelKind,
        symbol: Symbol,
        period: Period,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial_price = format!("{:.2}", market::quote(symbol, 0).price);
        let view = match kind {
            PanelKind::Watchlist => PanelView::Watchlist {
                query: cx.new(|cx| InputState::new(window, cx).placeholder("筛选列表 / 代码名称")),
                catalog_query: cx.new(|cx| InputState::new(window, cx).placeholder("搜索股票目录 / 代码名称")),
                import_text: cx.new(|cx| TextareaState::new(window, cx).auto_grow(4, 4).placeholder(
                    "粘贴股票代码或名称，每行一支或用逗号分隔\nAAPL 苹果\n00700 腾讯控股\nMSFT, TSLA")),
            },
            PanelKind::Chart => PanelView::Chart(cx.new(|cx| MarketChart::new(symbol, period, cx))),
            PanelKind::OrderEntry => PanelView::OrderEntry {
                price: cx.new(|cx| InputState::new(window, cx).default_value(initial_price.clone())),
                quantity: cx.new(|cx| InputState::new(window, cx).default_value("100")),
            },
            PanelKind::OrderDetails => PanelView::OrderDetails {
                min_amount: cx.new(|cx| InputState::new(window, cx).placeholder("最低")),
                max_amount: cx.new(|cx| InputState::new(window, cx).placeholder("最高")),
                min_quantity: cx.new(|cx| InputState::new(window, cx).placeholder("最低")),
                max_quantity: cx.new(|cx| InputState::new(window, cx).placeholder("最高")),
            },
            _ => PanelView::ReadOnly,
        };
        let mut subscriptions: Vec<_> = view
            .inputs()
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |_, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                })
            })
            .collect();
        if let PanelView::Watchlist { import_text, .. } = &view {
            subscriptions.push(
                cx.subscribe(import_text, |this, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.import_revision = this.import_revision.wrapping_add(1);
                        this.import_message = None;
                        this.refresh_import(cx);
                        cx.notify();
                    }
                }),
            );
        }
        if let PanelView::Chart(chart) = &view {
            subscriptions.push(
                cx.subscribe_in(chart, window, |this, _, event, window, cx| {
                    let ChartEvent::InspectDay {
                        symbol,
                        day_index,
                        tick,
                    } = *event;
                    let source = this.chart_entity().read(cx).data_source();
                    match history::open(
                        this.history.as_ref(),
                        symbol,
                        day_index,
                        tick,
                        source,
                        window,
                        cx,
                    ) {
                        Ok(view) => {
                            this.history = Some(view);
                            this.feedback = None;
                        }
                        Err(error) => {
                            this.feedback = Some((error.to_string(), true));
                        }
                    }
                    cx.notify();
                }),
            );
            subscriptions.push(cx.on_release(|this, cx| {
                if let Some(history) = this.history.take() {
                    history.close(cx);
                }
            }));
        }
        Self {
            view,
            legacy_content: ContentSnapshot::new(initial_price),
            kind,
            symbol,
            period,
            tick: 0,
            history: None,
            stock_list_page: StockListPage::List,
            import_preview: stock_import::parse("", &[]),
            import_message: None,
            import_loading: false,
            import_revision: 0,
            import_request: 0,
            members: Symbol::ALL.to_vec(),
            side: Side::Buy,
            sort: 0,
            filter_enabled: false,
            filter_direction: 0,
            large_trades_only: false,
            book_queue: false,
            feedback: None,
            expanded_news: None,
            pending_price: None,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn set_market(
        &mut self,
        symbol: Symbol,
        period: Period,
        tick: u64,
        cx: &mut Context<Self>,
    ) {
        if symbol != self.symbol {
            if matches!(self.view, PanelView::OrderEntry { .. }) {
                self.pending_price = Some(market::quote(symbol, tick).price);
            }
            self.feedback = None;
        }
        self.symbol = symbol;
        self.period = period;
        self.tick = tick;
        if let PanelView::Chart(chart) = &self.view {
            chart.update(cx, |chart, cx| chart.set_market(symbol, period, tick, cx));
        }
        cx.notify();
    }

    pub(crate) fn snapshot(&self, cx: &App) -> serde_json::Value {
        let mut saved = self.legacy_content.clone();
        match &self.view {
            PanelView::Watchlist { query, .. } => saved.query = query.read(cx).value().to_string(),
            PanelView::Chart(chart) => saved.chart = chart.read(cx).viewport(),
            PanelView::OrderEntry { price, quantity } => {
                saved.price = price.read(cx).value().to_string();
                saved.quantity = quantity.read(cx).value().to_string();
            }
            PanelView::OrderDetails { .. } => saved.order_filter = self.filter_snapshot(cx),
            PanelView::ReadOnly => {}
        }
        saved.side = self.side;
        saved.sort = self.sort;
        saved.large_trades_only = self.large_trades_only;
        saved.book_queue = self.book_queue;
        saved.members = self.members.clone();
        serde_json::to_value(saved).expect("panel snapshots contain only serializable values")
    }

    /// Decode and validate before mutating any live entity. Invalid known
    /// content is reported to the workspace, never silently replaced by defaults.
    pub(crate) fn restore(
        &mut self,
        value: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let Some(saved) = ContentSnapshot::decode(value)? else {
            return Ok(());
        };
        match &self.view {
            PanelView::Watchlist { query, .. } => {
                query.update(cx, |input, cx| {
                    input.set_value(saved.query.clone(), window, cx)
                });
            }
            PanelView::Chart(chart) => {
                chart.update(cx, |chart, cx| chart.restore_viewport(saved.chart, cx));
            }
            PanelView::OrderEntry { price, quantity } => {
                price.update(cx, |input, cx| {
                    input.set_value(saved.price.clone(), window, cx)
                });
                quantity.update(cx, |input, cx| {
                    input.set_value(saved.quantity.clone(), window, cx)
                });
            }
            PanelView::OrderDetails {
                min_amount,
                max_amount,
                min_quantity,
                max_quantity,
            } => {
                for (input, value) in [
                    (min_amount, &saved.order_filter.min_amount),
                    (max_amount, &saved.order_filter.max_amount),
                    (min_quantity, &saved.order_filter.min_quantity),
                    (max_quantity, &saved.order_filter.max_quantity),
                ] {
                    input.update(cx, |input, cx| input.set_value(value.clone(), window, cx));
                }
            }
            PanelView::ReadOnly => {}
        }
        self.side = saved.side;
        self.sort = saved.sort;
        self.filter_enabled = saved.order_filter.enabled;
        self.filter_direction = saved.order_filter.direction;
        self.large_trades_only = saved.large_trades_only;
        self.book_queue = saved.book_queue;
        self.members = saved.members.clone();
        if matches!(self.view, PanelView::Watchlist { .. }) {
            self.leave_stock_import(window, cx);
        }
        self.pending_price = None;
        self.legacy_content = saved;
        cx.notify();
        Ok(())
    }

    fn chart_entity(&self) -> &Entity<MarketChart> {
        match &self.view {
            PanelView::Chart(chart) => chart,
            _ => unreachable!("chart control on non-chart panel"),
        }
    }
    fn query(&self) -> &Entity<InputState> {
        match &self.view {
            PanelView::Watchlist { query, .. } => query,
            _ => unreachable!("query on non-list panel"),
        }
    }
    fn catalog_query(&self) -> &Entity<InputState> {
        match &self.view {
            PanelView::Watchlist { catalog_query, .. } => catalog_query,
            _ => unreachable!("catalog on non-list panel"),
        }
    }
    fn import_text(&self) -> &Entity<TextareaState> {
        match &self.view {
            PanelView::Watchlist { import_text, .. } => import_text,
            _ => unreachable!("import on non-list panel"),
        }
    }
    fn price(&self) -> &Entity<InputState> {
        match &self.view {
            PanelView::OrderEntry { price, .. } => price,
            _ => unreachable!("price on non-entry panel"),
        }
    }
    fn quantity(&self) -> &Entity<InputState> {
        match &self.view {
            PanelView::OrderEntry { quantity, .. } => quantity,
            _ => unreachable!("quantity on non-entry panel"),
        }
    }
    fn min_amount(&self) -> &Entity<InputState> {
        match &self.view {
            PanelView::OrderDetails { min_amount, .. } => min_amount,
            _ => unreachable!("filter on non-details panel"),
        }
    }
    fn max_amount(&self) -> &Entity<InputState> {
        match &self.view {
            PanelView::OrderDetails { max_amount, .. } => max_amount,
            _ => unreachable!("filter on non-details panel"),
        }
    }
    fn min_quantity(&self) -> &Entity<InputState> {
        match &self.view {
            PanelView::OrderDetails { min_quantity, .. } => min_quantity,
            _ => unreachable!("filter on non-details panel"),
        }
    }
    fn max_quantity(&self) -> &Entity<InputState> {
        match &self.view {
            PanelView::OrderDetails { max_quantity, .. } => max_quantity,
            _ => unreachable!("filter on non-details panel"),
        }
    }

    fn select_symbol(&mut self, symbol: Symbol, cx: &mut Context<Self>) {
        self.set_market(symbol, self.period, self.tick, cx);
        cx.emit(ContentEvent::SymbolSelected(symbol));
    }
}

impl Render for PanelContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(price) = self.pending_price.take() {
            self.price().update(cx, |input, cx| {
                input.set_value(format!("{price:.2}"), window, cx)
            });
        }
        let body = match self.kind {
            PanelKind::Watchlist => self.watchlist(cx),
            PanelKind::Chart => self.chart(cx),
            PanelKind::Quote => self.quote(),
            PanelKind::OrderBook => self.order_book(cx),
            PanelKind::Trades => self.trades(),
            PanelKind::TradeDetails => self.trade_details(cx),
            PanelKind::OrderDetails => self.order_details(cx),
            PanelKind::News => self.news(cx),
            PanelKind::OrderEntry => self.order_entry(cx),
            PanelKind::Orders => self.orders(cx),
            PanelKind::Positions => self.positions(cx),
        };
        div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(rgb(0xffffff))
            .text_color(rgb(INK))
            .text_size(px(13.))
            .child(body)
    }
}
