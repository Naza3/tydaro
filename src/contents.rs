//! Business panels share a deterministic quote stream and a local demo ledger.
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
    },
    prelude::*,
    *,
};
use serde::{Deserialize, Serialize};

use crate::{
    chart::{ChartEvent, ChartViewport, MarketChart},
    history::{self, HistoryWindow},
    market::{self, Period, Symbol},
    model::PanelKind,
};

const INK: u32 = 0x233348;
const MUTED: u32 = 0x85909f;
const LINE: u32 = 0xe8ecf0;
const TEAL: u32 = 0x009b9a;
const RED: u32 = 0xef4770;

#[derive(Clone, Debug)]
pub enum ContentEvent {
    SymbolSelected(Symbol),
    PeriodChanged(Period),
    OrderSubmitted,
    ListChanged,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
enum Side {
    Buy,
    Sell,
}
impl Side {
    fn label(self) -> &'static str {
        if self == Self::Buy {
            "买入"
        } else {
            "卖出"
        }
    }
    fn color(self) -> u32 {
        if self == Self::Buy { RED } else { TEAL }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
enum OrderStatus {
    Pending,
    Filled,
    Cancelled,
}
impl OrderStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Pending => "待模拟成交",
            Self::Filled => "模拟已成交",
            Self::Cancelled => "已撤销",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct DemoOrder {
    id: u64,
    symbol: Symbol,
    side: Side,
    price: f64,
    quantity: u64,
    status: OrderStatus,
}
#[derive(Clone, Serialize, Deserialize)]
struct DemoPosition {
    symbol: Symbol,
    quantity: u64,
    cost: f64,
}

/// Session-only accounting. US and HK demonstration balances are independent.
#[derive(Clone)]
struct DemoBroker {
    orders: Vec<DemoOrder>,
    positions: Vec<DemoPosition>,
    usd: f64,
    hkd: f64,
    next_id: u64,
}
impl Global for DemoBroker {}
impl Default for DemoBroker {
    fn default() -> Self {
        Self {
            orders: vec![DemoOrder {
                id: 1001,
                symbol: Symbol::Aapl,
                side: Side::Buy,
                price: 335.2,
                quantity: 100,
                status: OrderStatus::Filled,
            }],
            positions: vec![
                DemoPosition {
                    symbol: Symbol::Aapl,
                    quantity: 100,
                    cost: 335.2,
                },
                DemoPosition {
                    symbol: Symbol::Nvda,
                    quantity: 80,
                    cost: 179.8,
                },
            ],
            usd: 100_000.,
            hkd: 500_000.,
            next_id: 1002,
        }
    }
}
impl DemoBroker {
    fn available_cash(&self, symbol: Symbol) -> f64 {
        let balance = if symbol.market() == "HK" {
            self.hkd
        } else {
            self.usd
        };
        balance
            - self
                .orders
                .iter()
                .filter(|o| {
                    o.symbol.market() == symbol.market()
                        && o.side == Side::Buy
                        && o.status == OrderStatus::Pending
                })
                .map(|o| o.price * o.quantity as f64)
                .sum::<f64>()
    }
    fn available_shares(&self, symbol: Symbol) -> u64 {
        let held = self
            .positions
            .iter()
            .find(|p| p.symbol == symbol)
            .map_or(0, |p| p.quantity);
        held.saturating_sub(
            self.orders
                .iter()
                .filter(|o| {
                    o.symbol == symbol && o.side == Side::Sell && o.status == OrderStatus::Pending
                })
                .map(|o| o.quantity)
                .sum(),
        )
    }
    fn submit(
        &mut self,
        symbol: Symbol,
        side: Side,
        price: f64,
        quantity: u64,
    ) -> Result<u64, &'static str> {
        if !price.is_finite() || price <= 0. || price > 1_000_000. {
            return Err("请输入有效价格（0 至 1,000,000）。");
        }
        if quantity == 0 || quantity > 1_000_000 {
            return Err("请输入 1 至 1,000,000 的整数股数。");
        }
        if side == Side::Buy && price * quantity as f64 > self.available_cash(symbol) {
            return Err("模拟可用资金不足，请减少数量或撤销已有委托。");
        }
        if side == Side::Sell && quantity > self.available_shares(symbol) {
            return Err("模拟可卖数量不足，不支持卖空。");
        }
        let id = self.next_id;
        self.next_id += 1;
        self.orders.push(DemoOrder {
            id,
            symbol,
            side,
            price,
            quantity,
            status: OrderStatus::Pending,
        });
        Ok(id)
    }
    fn cancel(&mut self, id: u64) {
        if let Some(order) = self
            .orders
            .iter_mut()
            .find(|o| o.id == id && o.status == OrderStatus::Pending)
        {
            order.status = OrderStatus::Cancelled;
        }
    }
    fn fill(&mut self, id: u64) {
        let Some(order) = self
            .orders
            .iter_mut()
            .find(|o| o.id == id && o.status == OrderStatus::Pending)
        else {
            return;
        };
        order.status = OrderStatus::Filled;
        let order = order.clone();
        let value = order.price * order.quantity as f64;
        let balance = if order.symbol.market() == "HK" {
            &mut self.hkd
        } else {
            &mut self.usd
        };
        if order.side == Side::Buy {
            *balance -= value;
            if let Some(position) = self.positions.iter_mut().find(|p| p.symbol == order.symbol) {
                let quantity = position.quantity + order.quantity;
                position.cost =
                    (position.cost * position.quantity as f64 + value) / quantity as f64;
                position.quantity = quantity;
            } else {
                self.positions.push(DemoPosition {
                    symbol: order.symbol,
                    quantity: order.quantity,
                    cost: order.price,
                });
            }
        } else {
            *balance += value;
            if let Some(position) = self.positions.iter_mut().find(|p| p.symbol == order.symbol) {
                position.quantity = position.quantity.saturating_sub(order.quantity);
            }
            self.positions.retain(|p| p.quantity > 0);
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct OrderFilterSnapshot {
    enabled: bool,
    min_amount: String,
    max_amount: String,
    min_quantity: String,
    max_quantity: String,
    direction: u8,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct OrderRange {
    min_amount: f64,
    max_amount: f64,
    min_quantity: f64,
    max_quantity: f64,
}
impl OrderRange {
    fn parse(saved: &OrderFilterSnapshot) -> Result<Self, &'static str> {
        fn bound(value: &str, fallback: f64) -> Result<f64, &'static str> {
            if value.trim().is_empty() {
                return Ok(fallback);
            }
            value
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite() && *value >= 0.)
                .ok_or("筛选值须为非负数字")
        }
        let range = Self {
            min_amount: bound(&saved.min_amount, 0.)? * 10_000.,
            max_amount: bound(&saved.max_amount, f64::MAX / 10_000.)? * 10_000.,
            min_quantity: bound(&saved.min_quantity, 0.)?,
            max_quantity: bound(&saved.max_quantity, f64::MAX)?,
        };
        if range.min_amount > range.max_amount || range.min_quantity > range.max_quantity {
            return Err("最低值不能大于最高值");
        }
        if !range.min_amount.is_finite() || !range.max_amount.is_finite() {
            return Err("筛选金额超出范围");
        }
        Ok(range)
    }
    fn contains(self, order: &market::MarketOrder) -> bool {
        order.amount >= self.min_amount
            && order.amount <= self.max_amount
            && order.size as f64 >= self.min_quantity
            && order.size as f64 <= self.max_quantity
    }
}

#[derive(Serialize, Deserialize)]
struct ContentSnapshot {
    query: String,
    price: String,
    quantity: String,
    side: Side,
    sort: u8,
    chart: ChartViewport,
    #[serde(default)]
    order_filter: OrderFilterSnapshot,
    #[serde(default)]
    large_trades_only: bool,
    #[serde(default)]
    book_queue: bool,
    #[serde(default = "default_members", deserialize_with = "read_members")]
    members: Vec<Symbol>,
}

fn default_members() -> Vec<Symbol> {
    Symbol::ALL.to_vec()
}

fn read_members<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Symbol>, D::Error> {
    let mut unique = Vec::new();
    for symbol in Vec::<Symbol>::deserialize(deserializer)? {
        if !unique.contains(&symbol) {
            unique.push(symbol);
        }
    }
    Ok(unique)
}

pub struct PanelContent {
    kind: PanelKind,
    symbol: Symbol,
    period: Period,
    tick: u64,
    chart: Entity<MarketChart>,
    history: Option<HistoryWindow>,
    query: Entity<InputState>,
    catalog_query: Entity<InputState>,
    adding_stocks: bool,
    members: Vec<Symbol>,
    price: Entity<InputState>,
    quantity: Entity<InputState>,
    side: Side,
    sort: u8,
    filter_enabled: bool,
    filter_direction: u8,
    min_amount: Entity<InputState>,
    max_amount: Entity<InputState>,
    min_quantity: Entity<InputState>,
    max_quantity: Entity<InputState>,
    large_trades_only: bool,
    book_queue: bool,
    feedback: Option<(String, bool)>,
    expanded_news: Option<usize>,
    pending_price: Option<f64>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<ContentEvent> for PanelContent {}

impl PanelContent {
    pub fn new(
        kind: PanelKind,
        symbol: Symbol,
        period: Period,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        if !cx.has_global::<DemoBroker>() {
            cx.set_global(DemoBroker::default());
        }
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("筛选列表 / 代码名称"));
        let catalog_query =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索股票目录 / 代码名称"));
        let price = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(format!("{:.2}", market::quote(symbol, 0).price))
        });
        let quantity = cx.new(|cx| InputState::new(window, cx).default_value("100"));
        let min_amount = cx.new(|cx| InputState::new(window, cx).placeholder("最低"));
        let max_amount = cx.new(|cx| InputState::new(window, cx).placeholder("最高"));
        let min_quantity = cx.new(|cx| InputState::new(window, cx).placeholder("最低"));
        let max_quantity = cx.new(|cx| InputState::new(window, cx).placeholder("最高"));
        let mut subscriptions: Vec<_> = [
            &query,
            &catalog_query,
            &price,
            &quantity,
            &min_amount,
            &max_amount,
            &min_quantity,
            &max_quantity,
        ]
        .into_iter()
        .map(|input| {
            cx.subscribe(input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
        })
        .collect();
        let chart = cx.new(|cx| MarketChart::new(symbol, period, cx));
        subscriptions.push(
            cx.subscribe_in(&chart, window, |this, _, event, window, cx| {
                let ChartEvent::InspectDay {
                    symbol,
                    day_index,
                    tick,
                } = *event;
                match history::open(this.history.as_ref(), symbol, day_index, tick, window, cx) {
                    Ok(view) => {
                        this.history = Some(view);
                        this.feedback = None;
                        cx.notify();
                    }
                    Err(error) => {
                        this.feedback = Some((error.to_string(), true));
                        cx.notify();
                    }
                }
            }),
        );
        subscriptions.push(cx.on_release(|this, cx| {
            if let Some(history) = this.history.take() {
                history.close(cx);
            }
        }));
        Self {
            kind,
            symbol,
            period,
            tick: 0,
            chart,
            history: None,
            query,
            catalog_query,
            adding_stocks: false,
            members: default_members(),
            price,
            quantity,
            side: Side::Buy,
            sort: 0,
            filter_enabled: false,
            filter_direction: 0,
            min_amount,
            max_amount,
            min_quantity,
            max_quantity,
            large_trades_only: false,
            book_queue: false,
            feedback: None,
            expanded_news: None,
            pending_price: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_market(
        &mut self,
        symbol: Symbol,
        period: Period,
        tick: u64,
        cx: &mut Context<Self>,
    ) {
        if symbol != self.symbol {
            self.pending_price = Some(market::quote(symbol, tick).price);
            self.feedback = None;
        }
        self.symbol = symbol;
        self.period = period;
        self.tick = tick;
        self.chart
            .update(cx, |chart, cx| chart.set_market(symbol, period, tick, cx));
        cx.notify();
    }

    pub fn snapshot(&self, cx: &App) -> serde_json::Value {
        serde_json::to_value(ContentSnapshot {
            query: self.query.read(cx).value().to_string(),
            price: self.price.read(cx).value().to_string(),
            quantity: self.quantity.read(cx).value().to_string(),
            side: self.side,
            sort: self.sort,
            chart: self.chart.read(cx).viewport(),
            order_filter: self.filter_snapshot(cx),
            large_trades_only: self.large_trades_only,
            book_queue: self.book_queue,
            members: self.members.clone(),
        })
        .unwrap_or(serde_json::Value::Null)
    }

    pub fn restore(
        &mut self,
        value: &serde_json::Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Ok(saved) = serde_json::from_value::<ContentSnapshot>(value.clone()) {
            self.query
                .update(cx, |input, cx| input.set_value(saved.query, window, cx));
            self.price
                .update(cx, |input, cx| input.set_value(saved.price, window, cx));
            self.quantity
                .update(cx, |input, cx| input.set_value(saved.quantity, window, cx));
            self.side = saved.side;
            self.sort = saved.sort.min(2);
            self.filter_enabled = saved.order_filter.enabled;
            self.filter_direction = saved.order_filter.direction.min(2);
            self.large_trades_only = saved.large_trades_only;
            self.book_queue = saved.book_queue;
            self.members = saved.members;
            self.adding_stocks = false;
            for (input, value) in [
                (&self.min_amount, saved.order_filter.min_amount),
                (&self.max_amount, saved.order_filter.max_amount),
                (&self.min_quantity, saved.order_filter.min_quantity),
                (&self.max_quantity, saved.order_filter.max_quantity),
            ] {
                input.update(cx, |input, cx| input.set_value(value, window, cx));
            }
            self.chart
                .update(cx, |chart, cx| chart.restore_viewport(saved.chart, cx));
            self.pending_price = None;
            cx.notify();
        }
    }

    fn select_symbol(&mut self, symbol: Symbol, cx: &mut Context<Self>) {
        self.set_market(symbol, self.period, self.tick, cx);
        cx.emit(ContentEvent::SymbolSelected(symbol));
    }

    fn show_stock_catalog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.adding_stocks = true;
        self.catalog_query.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    fn stock_catalog(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.catalog_query.read(cx).value().trim().to_lowercase();
        let symbols: Vec<_> = Symbol::ALL
            .into_iter()
            .filter(|symbol| {
                symbol.code().to_lowercase().contains(&query) || symbol.name().contains(&query)
            })
            .collect();
        let empty = symbols.is_empty();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px_2()
                    .pt_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().font_weight(FontWeight::MEDIUM).child("添加股票"))
                    .child(
                        Button::new("stock-list-back")
                            .label("返回列表")
                            .small()
                            .ghost()
                            .text_color(rgb(TEAL))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.adding_stocks = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div().p_2().child(
                    Input::new(&self.catalog_query)
                        .id("stock-list-catalog-search")
                        .small()
                        .w_full(),
                ),
            )
            .child(
                div()
                    .id("stock-list-catalog-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(symbols.into_iter().map(|symbol| {
                        let exists = self.members.contains(&symbol);
                        div()
                            .h(px(53.))
                            .px_2()
                            .flex()
                            .items_center()
                            .gap_2()
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().truncate().child(symbol.name()))
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(10.))
                                            .text_color(rgb(MUTED))
                                            .child(format!(
                                                "{}  {}",
                                                symbol.market(),
                                                symbol.code()
                                            )),
                                    ),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "stock-list-add-{}",
                                    symbol.code()
                                )))
                                .label(if exists { "已添加" } else { "添加" })
                                .small()
                                .ghost()
                                .flex_shrink_0()
                                .text_color(rgb(TEAL))
                                .disabled(exists)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        if !this.members.contains(&symbol) {
                                            this.members.push(symbol);
                                            cx.emit(ContentEvent::ListChanged);
                                            cx.notify();
                                        }
                                    },
                                )),
                            )
                    }))
                    .when(empty, |d| {
                        d.child(
                            div()
                                .p_4()
                                .text_color(rgb(MUTED))
                                .child("模拟股票目录中没有匹配结果"),
                        )
                    }),
            )
            .child(
                div()
                    .px_2()
                    .py_2()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .child(format!("模拟股票目录 · 当前列表 {} 支", self.members.len())),
            )
            .into_any_element()
    }

    fn watchlist(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.adding_stocks {
            return self.stock_catalog(cx);
        }
        let query = self.query.read(cx).value().trim().to_lowercase();
        let mut symbols: Vec<_> = self
            .members
            .iter()
            .copied()
            .filter(|s| s.code().to_lowercase().contains(&query) || s.name().contains(&query))
            .collect();
        if self.sort > 0 {
            symbols.sort_by(|a, b| {
                market::quote(*a, self.tick)
                    .change_pct
                    .total_cmp(&market::quote(*b, self.tick).change_pct)
            });
            if self.sort == 1 {
                symbols.reverse();
            }
        }
        let count = symbols.len();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .p_2()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.query)
                                .id("watchlist-search")
                                .small()
                                .w_full(),
                        ),
                    )
                    .child(
                        Button::new("stock-list-add")
                            .label("添加")
                            .tooltip("添加股票到当前列表")
                            .small()
                            .ghost()
                            .flex_shrink_0()
                            .text_color(rgb(TEAL))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.show_stock_catalog(window, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .h(px(29.))
                    .px_2()
                    .flex()
                    .items_center()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .text_color(rgb(MUTED))
                    .child(div().flex_1().min_w_0().truncate().child("名称 / 代码"))
                    .child(
                        div()
                            .w(px(58.))
                            .flex_shrink_0()
                            .text_right()
                            .child("最新价"),
                    )
                    .child(
                        control(
                            "watchlist-sort",
                            match self.sort {
                                1 => "涨跌幅 ↓",
                                2 => "涨跌幅 ↑",
                                _ => "涨跌幅 ↕",
                            },
                            false,
                        )
                        .w(px(58.))
                        .px_0()
                        .flex_shrink_0()
                        .text_right()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sort = (this.sort + 1) % 3;
                            cx.notify();
                        }))
                        .test_support(),
                    )
                    .child(div().w(px(26.)).flex_shrink_0()),
            )
            .child(
                div()
                    .id("watchlist-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(symbols.into_iter().map(|symbol| {
                        let quote = market::quote(symbol, self.tick);
                        div()
                            .id(SharedString::from(format!("symbol-{}", symbol.code())))
                            .h(px(53.))
                            .px_2()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0xf0f2f5))
                            .bg(rgb(if self.symbol == symbol {
                                0xe9f7f7
                            } else {
                                0xffffff
                            }))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0xf1f8f9)))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(symbol.name()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_size(px(10.))
                                            .text_color(rgb(MUTED))
                                            .child(format!(
                                                "{}  {}",
                                                symbol.market(),
                                                symbol.code()
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .w(px(58.))
                                    .flex_shrink_0()
                                    .text_right()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(movement(quote.change))
                                    .child(format!("{:.3}", quote.price)),
                            )
                            .child(
                                div()
                                    .w(px(58.))
                                    .flex_shrink_0()
                                    .text_right()
                                    .text_color(movement(quote.change))
                                    .child(format!("{:+.2}%", quote.change_pct)),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "stock-list-remove-{}",
                                    symbol.code()
                                )))
                                .label("×")
                                .tooltip(format!("从当前列表移除 {}", symbol.code()))
                                .small()
                                .ghost()
                                .w(px(26.))
                                .px_0()
                                .flex_shrink_0()
                                .text_color(rgb(MUTED))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.members.retain(|member| *member != symbol);
                                        cx.emit(ContentEvent::ListChanged);
                                        cx.notify();
                                    },
                                )),
                            )
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.select_symbol(symbol, cx)),
                            )
                            .test_support()
                    }))
                    .when(count == 0, |d| {
                        d.child(
                            div()
                                .p_4()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .text_color(rgb(MUTED))
                                .child(if self.members.is_empty() {
                                    "列表为空，添加股票开始关注"
                                } else {
                                    "列表中没有匹配的股票"
                                })
                                .when(self.members.is_empty(), |d| {
                                    d.child(
                                        Button::new("stock-list-empty-add")
                                            .label("添加股票")
                                            .small()
                                            .ghost()
                                            .text_color(rgb(TEAL))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.show_stock_catalog(window, cx);
                                            })),
                                    )
                                }),
                        )
                    }),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(if query.is_empty() {
                        format!("{} 支股票 · 点击联动同组组件", self.members.len())
                    } else {
                        format!("显示 {} / {} 支股票", count, self.members.len())
                    }),
            )
            .into_any_element()
    }

    fn chart(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("chart-periods")
                    .h(px(31.))
                    .flex_shrink_0()
                    .px_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .overflow_x_scroll()
                    .children(Period::ALL.into_iter().map(|period| {
                        control(
                            SharedString::from(format!("period-{}", period.label())),
                            period.label(),
                            period == self.period,
                        )
                        .flex_shrink_0()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_market(this.symbol, period, this.tick, cx);
                            cx.emit(ContentEvent::PeriodChanged(period));
                        }))
                        .test_support()
                    }))
                    .child(div().flex_1())
                    .child(
                        control("chart-zoom-out", "−", false)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.chart
                                    .update(cx, |chart, cx| chart.zoom_step(false, cx));
                            }))
                            .test_support(),
                    )
                    .child(
                        control("chart-zoom-in", "+", false)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.chart.update(cx, |chart, cx| chart.zoom_step(true, cx));
                            }))
                            .test_support(),
                    )
                    .child(
                        control("chart-reset", "复位", false)
                            .flex_shrink_0()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.chart.update(cx, |chart, cx| chart.reset(cx));
                            }))
                            .test_support(),
                    ),
            )
            .when_some(self.feedback.as_ref(), |view, (message, _)| {
                view.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_color(rgb(RED))
                        .child(message.clone()),
                )
            })
            .child(div().flex_1().min_h_0().child(self.chart.clone()))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .child(if self.period == Period::Day {
                        "单击查看数据 · ← → 逐日定位 · 双击查看当日分时 · Esc 退出定位"
                    } else {
                        "单击查看数据 · 滚轮缩放 · 拖动平移 · Esc 退出定位"
                    }),
            )
            .into_any_element()
    }

    fn quote(&self) -> AnyElement {
        let q = market::quote(self.symbol, self.tick);
        let price_color = |price| movement(price - q.previous_close);
        div()
            .id("quote-scroll")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .px_2()
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .text_color(movement(q.change))
                    .child(
                        div()
                            .text_size(px(17.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{:.3}", q.price)),
                    )
                    .child(format!("{:+.3}  {:+.2}%", q.change, q.change_pct)),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .child(metric_colored(
                        "最高",
                        format!("{:.3}", q.high),
                        price_color(q.high),
                        "开盘",
                        format!("{:.3}", q.open),
                        price_color(q.open),
                    ))
                    .child(metric_colored(
                        "最低",
                        format!("{:.3}", q.low),
                        price_color(q.low),
                        "昨收",
                        format!("{:.3}", q.previous_close),
                        rgb(INK),
                    ))
                    .child(metric(
                        "成交量",
                        compact(q.volume as f64),
                        "成交额",
                        compact(q.turnover),
                    ))
                    .child(metric(
                        "量比",
                        format!("{:.2}", q.volume_ratio),
                        "换手率",
                        format!("{:.2}%", q.turnover_rate),
                    ))
                    .child(metric(
                        "振幅",
                        format!("{:.2}%", (q.high - q.low) / q.previous_close * 100.),
                        "均价",
                        format!("{:.3}", q.turnover / q.volume.max(1) as f64),
                    ))
                    .child(metric("涨停", "—".into(), "跌停", "—".into()))
                    .child(metric_colored(
                        "主动买",
                        compact(q.buy_volume as f64),
                        rgb(RED),
                        "主动卖",
                        compact(q.sell_volume as f64),
                        rgb(TEAL),
                    ))
                    .child(metric(
                        "市盈(TTM)",
                        format!("{:.2}", q.pe_ttm),
                        "每股盈利",
                        format!("{:.3}", q.price / q.pe_ttm),
                    ))
                    .child(metric(
                        "总市值",
                        compact(q.market_cap),
                        "流通值",
                        compact(q.float_market_cap),
                    ))
                    .child(metric(
                        "总股本",
                        compact(q.total_shares as f64),
                        "流通股",
                        compact(q.float_shares as f64),
                    )),
            )
            .child(
                div()
                    .px_2()
                    .h(px(18.))
                    .flex()
                    .items_center()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(format!("{} · 量 / 股本：股 · 模拟", currency(self.symbol))),
            )
            .into_any_element()
    }

    fn order_book(&self, cx: &mut Context<Self>) -> AnyElement {
        let (bids, asks) = market::order_book(self.symbol, self.tick);
        let buy_total: u64 = bids.iter().map(|l| l.size).sum();
        let sell_total: u64 = asks.iter().map(|l| l.size).sum();
        let ratio = buy_total as f32 / (buy_total + sell_total).max(1) as f32;
        let max_size = bids
            .iter()
            .chain(asks.iter())
            .map(|l| l.size)
            .max()
            .unwrap_or(1) as f32;
        let pending: Vec<_> = market::market_orders(self.symbol, self.tick)
            .into_iter()
            .filter(|order| order.canceled_at.is_none())
            .collect();
        let q = market::quote(self.symbol, self.tick);
        let mut depth = div()
            .id("book-scroll")
            .flex_1()
            .min_h(px(40.))
            .overflow_scroll();
        if self.book_queue {
            let max = pending.iter().map(|order| order.size).max().unwrap_or(1) as f32;
            depth = depth
                .child(dense_columns(
                    &[
                        ("方向", 36.),
                        ("价格", 75.),
                        ("委托量（股）", 95.),
                        ("时间", 70.),
                    ],
                    true,
                ))
                .children(pending.iter().map(|order| {
                    div()
                        .h(px(20.))
                        .px_2()
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xf3f5f7))
                        .child(
                            div()
                                .w(px(36.))
                                .flex_shrink_0()
                                .text_color(rgb(if order.buy { RED } else { TEAL }))
                                .child(if order.buy { "买" } else { "卖" }),
                        )
                        .child(
                            div()
                                .w(px(75.))
                                .flex_shrink_0()
                                .child(format!("{:.3}", order.price)),
                        )
                        .child(amount_cell(
                            order.size as f64,
                            order.size as f32 / max,
                            if order.buy { RED } else { TEAL },
                            95.,
                        ))
                        .child(
                            div()
                                .w(px(70.))
                                .text_right()
                                .text_color(rgb(MUTED))
                                .child(order.time.clone()),
                        )
                }));
        } else {
            depth = depth
                .child(book_section("卖盘", TEAL))
                .children(asks.iter().enumerate().map(|(index, level)| {
                    book_row(
                        format!("卖 {}", index + 1),
                        level.price,
                        level.size,
                        max_size,
                        TEAL,
                    )
                }))
                .child(book_section("买盘", RED))
                .children(bids.iter().enumerate().map(|(index, level)| {
                    book_row(
                        format!("买 {}", index + 1),
                        level.price,
                        level.size,
                        max_size,
                        RED,
                    )
                }));
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(24.))
                    .px_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(
                        control("book-depth", "买卖十档", !self.book_queue)
                            .py(px(2.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.book_queue = false;
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .child(
                        control("book-queue", "委托队列", self.book_queue)
                            .py(px(2.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.book_queue = true;
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(MUTED))
                            .child(format!("股 · {}", currency(self.symbol))),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .h(px(18.))
                    .items_center()
                    .flex_shrink_0()
                    .flex()
                    .justify_between()
                    .text_size(px(11.))
                    .child(
                        div()
                            .text_color(rgb(RED))
                            .child(format!("买盘 {:.1}%", ratio * 100.)),
                    )
                    .child(
                        div()
                            .text_color(rgb(TEAL))
                            .child(format!("卖盘 {:.1}%", (1. - ratio) * 100.)),
                    ),
            )
            .child(
                div()
                    .mx_2()
                    .h(px(2.))
                    .flex_shrink_0()
                    .flex()
                    .bg(rgb(TEAL))
                    .child(div().h_full().w(relative(ratio)).bg(rgb(RED))),
            )
            .child(
                div()
                    .id("best-bid-ask")
                    .mx_2()
                    .flex_shrink_0()
                    .h(px(20.))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .px_1()
                            .flex()
                            .items_center()
                            .justify_between()
                            .bg(rgb(0xffeff3))
                            .text_color(rgb(RED))
                            .child("买一")
                            .child(format!("{:.3}", bids[0].price)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .px_1()
                            .flex()
                            .items_center()
                            .justify_between()
                            .bg(rgb(0xe8f8f5))
                            .text_color(rgb(TEAL))
                            .child("卖一")
                            .child(format!("{:.3}", asks[0].price)),
                    )
                    .test_support(),
            )
            .when(!self.book_queue, |d| {
                d.child(
                    div()
                        .h(px(20.))
                        .px_2()
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .text_color(rgb(MUTED))
                        .border_b_1()
                        .border_color(rgb(LINE))
                        .child(div().w(px(36.)).child("档位"))
                        .child(div().flex_1().text_right().child("价格"))
                        .child(div().w(px(90.)).text_right().child("数量（股）")),
                )
            })
            .child(depth)
            .child(
                div()
                    .px_2()
                    .h(px(18.))
                    .items_center()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .flex_shrink_0()
                    .text_size(px(11.))
                    .flex()
                    .justify_between()
                    .child(format!("均价 {:.3}", q.turnover / q.volume.max(1) as f64))
                    .child(
                        div()
                            .text_color(rgb(MUTED))
                            .child(format!("挂单 {} 笔", pending.len())),
                    ),
            )
            .into_any_element()
    }

    fn trades(&self) -> AnyElement {
        let q = market::quote(self.symbol, self.tick);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(20.))
                    .px_2()
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .text_color(rgb(MUTED))
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(div().w(px(58.)).child("时间"))
                    .child(
                        div()
                            .flex_1()
                            .text_right()
                            .child(format!("价格 {}", currency(self.symbol))),
                    )
                    .child(div().w(px(62.)).text_right().child("量（股）"))
                    .child(div().w(px(32.)).text_right().child("笔数")),
            )
            .child(
                div()
                    .id("trades-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(
                        market::trades(self.symbol, self.tick)
                            .into_iter()
                            .enumerate()
                            .map(|(index, trade)| {
                                div()
                                    .h(px(20.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .bg(rgb(if index % 5 == 0 { 0xf5f8fb } else { 0xffffff }))
                                    .child(
                                        div().w(px(58.)).text_color(rgb(MUTED)).child(trade.time),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_right()
                                            .text_color(movement(trade.price - q.previous_close))
                                            .child(format!("{:.3}", trade.price)),
                                    )
                                    .child(
                                        div()
                                            .w(px(62.))
                                            .text_right()
                                            .text_color(rgb(if trade.buy { RED } else { TEAL }))
                                            .child(trade.size.to_string()),
                                    )
                                    .child(
                                        div()
                                            .w(px(32.))
                                            .text_right()
                                            .text_color(rgb(MUTED))
                                            .child(trade.count.to_string()),
                                    )
                            }),
                    ),
            )
            .into_any_element()
    }

    fn trade_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let executions: Vec<_> = market::executions(self.symbol, self.tick)
            .into_iter()
            .filter(|trade| !self.large_trades_only || trade.amount >= 100_000.)
            .collect();
        let max = executions
            .iter()
            .map(|trade| {
                trade
                    .amount
                    .max(trade.buy_order_amount)
                    .max(trade.sell_order_amount)
            })
            .fold(1., f64::max);
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(24.))
                    .px_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(
                        control("flow-all", "全部成交", !self.large_trades_only)
                            .py(px(2.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.large_trades_only = false;
                                cx.notify();
                            }))
                            .test_support(),
                    )
                    .child(
                        control("flow-large", "大单 ≥ 10万", self.large_trades_only)
                            .py(px(2.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.large_trades_only = true;
                                cx.notify();
                            }))
                            .test_support(),
                    ),
            )
            .child(
                div()
                    .id("executions-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .child(
                        div()
                            .min_w(px(354.))
                            .child(dense_columns(
                                &[
                                    ("时间", 64.),
                                    ("价格", 66.),
                                    ("金额", 76.),
                                    ("买单", 66.),
                                    ("卖单", 66.),
                                ],
                                true,
                            ))
                            .when(executions.is_empty(), |d| {
                                d.child(
                                    div()
                                        .p_3()
                                        .text_color(rgb(MUTED))
                                        .child("当前没有满足条件的成交"),
                                )
                            })
                            .children(executions.iter().map(|trade| {
                                div()
                                    .h(px(20.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .border_b_1()
                                    .border_color(rgb(0xf3f5f7))
                                    .child(
                                        div()
                                            .w(px(64.))
                                            .flex_shrink_0()
                                            .text_color(rgb(MUTED))
                                            .child(trade.time.clone()),
                                    )
                                    .child(
                                        div()
                                            .w(px(66.))
                                            .flex_shrink_0()
                                            .text_right()
                                            .text_color(rgb(if trade.buy { RED } else { TEAL }))
                                            .child(format!("{:.3}", trade.price)),
                                    )
                                    .child(amount_cell(
                                        trade.amount,
                                        (trade.amount / max) as f32,
                                        if trade.buy { RED } else { TEAL },
                                        76.,
                                    ))
                                    .child(amount_cell(
                                        trade.buy_order_amount,
                                        (trade.buy_order_amount / max) as f32,
                                        RED,
                                        66.,
                                    ))
                                    .child(amount_cell(
                                        trade.sell_order_amount,
                                        (trade.sell_order_amount / max) as f32,
                                        TEAL,
                                        66.,
                                    ))
                            })),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .child(format!(
                        "{} · {} 笔 · 买单 / 卖单为对应委托金额",
                        currency(self.symbol),
                        executions.len()
                    )),
            )
            .into_any_element()
    }

    fn filter_snapshot(&self, cx: &App) -> OrderFilterSnapshot {
        OrderFilterSnapshot {
            enabled: self.filter_enabled,
            min_amount: self.min_amount.read(cx).value().to_string(),
            max_amount: self.max_amount.read(cx).value().to_string(),
            min_quantity: self.min_quantity.read(cx).value().to_string(),
            max_quantity: self.max_quantity.read(cx).value().to_string(),
            direction: self.filter_direction,
        }
    }

    fn order_details(&self, cx: &mut Context<Self>) -> AnyElement {
        let filter = self.filter_snapshot(cx);
        let range = OrderRange::parse(&filter);
        let orders: Vec<_> = market::market_orders(self.symbol, self.tick)
            .into_iter()
            .filter(|order| {
                (filter.direction == 0 || (filter.direction == 1) == order.buy)
                    && (!filter.enabled || range.as_ref().is_ok_and(|range| range.contains(order)))
            })
            .collect();
        let pending: Vec<_> = orders
            .iter()
            .filter(|order| order.canceled_at.is_none())
            .collect();
        let canceled: Vec<_> = orders
            .iter()
            .filter(|order| order.canceled_at.is_some())
            .collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("order-filter-toolbar")
                    .px_1()
                    .py_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_wrap()
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(LINE))
                    .child(
                        control(
                            "market-filter-enabled",
                            if self.filter_enabled {
                                "☑ 筛选"
                            } else {
                                "☐ 筛选"
                            },
                            self.filter_enabled,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.filter_enabled = !this.filter_enabled;
                            cx.notify();
                        }))
                        .test_support(),
                    )
                    .child(
                        div()
                            .px_1()
                            .text_color(rgb(MUTED))
                            .child(format!("金额（万 {}）", currency(self.symbol))),
                    )
                    .child(
                        Input::new(&self.min_amount)
                            .id("market-min-amount")
                            .small()
                            .w(px(66.)),
                    )
                    .child("–")
                    .child(
                        Input::new(&self.max_amount)
                            .id("market-max-amount")
                            .small()
                            .w(px(66.)),
                    )
                    .child(div().px_1().text_color(rgb(MUTED)).child("数量（股）"))
                    .child(
                        Input::new(&self.min_quantity)
                            .id("market-min-quantity")
                            .small()
                            .w(px(66.)),
                    )
                    .child("–")
                    .child(
                        Input::new(&self.max_quantity)
                            .id("market-max-quantity")
                            .small()
                            .w(px(66.)),
                    )
                    .children([(0, "全部"), (1, "买单"), (2, "卖单")].into_iter().map(
                        |(direction, label)| {
                            control(
                                SharedString::from(format!("market-direction-{direction}")),
                                label,
                                self.filter_direction == direction,
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.filter_direction = direction;
                                cx.notify();
                            }))
                            .test_support()
                        },
                    )),
            )
            .when(self.filter_enabled && range.is_err(), |d| {
                d.child(
                    div()
                        .px_2()
                        .py_1()
                        .text_color(rgb(RED))
                        .child(range.as_ref().unwrap_err().to_string()),
                )
            })
            .child(
                div()
                    .id("market-orders-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_x_scroll()
                    .child(
                        div()
                            .h_full()
                            .min_w(px(970.))
                            .flex()
                            .child(order_detail_table(
                                "pending-market-orders",
                                "挂单",
                                &pending,
                                false,
                            ))
                            .child(order_detail_table(
                                "canceled-market-orders",
                                "撤单",
                                &canceled,
                                true,
                            )),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .py_1()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(rgb(LINE))
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .child("市场委托为模拟数据 · 本地账户委托请查看“当日订单”"),
            )
            .into_any_element()
    }

    fn news(&self, cx: &mut Context<Self>) -> AnyElement {
        let items = [
            (
                "公司动态",
                "09:42",
                format!(
                    "{}季度财报观察：关注营收结构与经营现金流",
                    self.symbol.name()
                ),
                "这是一条用于验证资讯排版和联动的本地示例。后续接入数据服务后，正文、发布时间、来源与公司公告将由统一资讯接口提供。",
            ),
            (
                "市场观察",
                "09:18",
                "全球市场晨报：科技板块与主要指数表现".into(),
                "模拟资讯正文：工作区支持同时打开多个股票面板。点击股票列表可以联动图表、报价、盘口和资讯；固定到独立联动组后，可以并排比较不同标的。",
            ),
            (
                "产品说明",
                "08:50",
                "自定义你的工作台：拖动标签即可调整面板位置".into(),
                "将标签拖到其他面板的上、下、左、右边缘可插入分区，拖到中央可合并标签。通过面板菜单弹出独立窗口，布局会保存到本机。",
            ),
            (
                "公司公告",
                "昨天",
                format!("{}投资者关系资料与历史公告入口（示例）", self.symbol.name()),
                "本地演示不会打开外部交易服务，也不会提交真实订单。模拟委托只保存在本次应用会话中，用于检查买卖表单、订单与持仓的交互。",
            ),
        ];
        div()
            .id("news-scroll")
            .size_full()
            .overflow_y_scroll()
            .children(
                items
                    .into_iter()
                    .enumerate()
                    .map(|(index, (tag, time, title, body))| {
                        div()
                            .id(SharedString::from(format!("news-{index}")))
                            .p_3()
                            .border_b_1()
                            .border_color(rgb(LINE))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0xf8fafc)))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .mb_2()
                                    .child(badge(tag))
                                    .child(
                                        div().text_size(px(10.)).text_color(rgb(MUTED)).child(time),
                                    ),
                            )
                            .child(div().line_height(relative(1.55)).child(title))
                            .when(self.expanded_news == Some(index), |d| {
                                d.child(
                                    div()
                                        .mt_2()
                                        .text_size(px(11.))
                                        .line_height(relative(1.65))
                                        .text_color(rgb(MUTED))
                                        .child(body),
                                )
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.expanded_news = if this.expanded_news == Some(index) {
                                    None
                                } else {
                                    Some(index)
                                };
                                cx.notify();
                            }))
                            .test_support()
                    }),
            )
            .into_any_element()
    }

    fn submit_order(&mut self, cx: &mut Context<Self>) {
        let price = self.price.read(cx).value().trim().parse::<f64>();
        let quantity = self.quantity.read(cx).value().trim().parse::<u64>();
        let result = match (price, quantity) {
            (Ok(price), Ok(quantity)) => cx.update_global::<DemoBroker, _>(|broker, _| {
                broker.submit(self.symbol, self.side, price, quantity)
            }),
            _ => Err("请输入有效价格和整数股数。"),
        };
        self.feedback = Some(match result {
            Ok(id) => (
                format!("模拟委托 #{id} 已记录。在“当日订单”中可模拟成交或撤单。"),
                true,
            ),
            Err(message) => (message.into(), false),
        });
        cx.emit(ContentEvent::OrderSubmitted);
        cx.notify();
    }

    fn order_entry(&self, cx: &mut Context<Self>) -> AnyElement {
        let broker = cx.global::<DemoBroker>();
        let available_cash = broker.available_cash(self.symbol);
        let available_shares = broker.available_shares(self.symbol);
        let price = self.price.read(cx).value().parse::<f64>().unwrap_or(0.);
        let qty = self.quantity.read(cx).value().parse::<u64>().unwrap_or(0);
        let amount = if price.is_finite() && price > 0. {
            price * qty as f64
        } else {
            0.
        };
        div()
            .id("entry-scroll")
            .size_full()
            .overflow_y_scroll()
            .p_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mb_3()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                        "{} {}",
                        self.symbol.code(),
                        self.symbol.name()
                    )))
                    .child(badge("本地模拟")),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .mb_3()
                    .children([Side::Buy, Side::Sell].into_iter().map(|side| {
                        control(
                            if side == Side::Buy {
                                "order-buy"
                            } else {
                                "order-sell"
                            },
                            side.label(),
                            self.side == side,
                        )
                        .flex_1()
                        .text_center()
                        .text_color(rgb(side.color()))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.side = side;
                            this.feedback = None;
                            cx.notify();
                        }))
                        .test_support()
                    })),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        div().flex_1().min_w(px(125.)).child(label("限价")).child(
                            Input::new(&self.price)
                                .id("order-price")
                                .small()
                                .w_full()
                                .suffix(currency(self.symbol)),
                        ),
                    )
                    .child(
                        div().flex_1().min_w(px(125.)).child(label("数量")).child(
                            Input::new(&self.quantity)
                                .id("order-quantity")
                                .small()
                                .w_full()
                                .suffix("股"),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mt_2()
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .child(format!(
                        "可用 {:.2} {}",
                        available_cash,
                        currency(self.symbol)
                    ))
                    .child(format!("可卖 {} 股", available_shares)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mt_3()
                    .child(label("委托金额"))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(format!(
                        "{:.2} {}",
                        amount,
                        currency(self.symbol)
                    ))),
            )
            .child(
                div()
                    .id("submit-order")
                    .px_2()
                    .py_1()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .child(format!("提交模拟{}委托", self.side.label()))
                    .mt_3()
                    .w_full()
                    .text_center()
                    .bg(rgb(self.side.color()))
                    .text_color(rgb(0xffffff))
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|this, _, _, cx| this.submit_order(cx)))
                    .test_support(),
            )
            .when_some(self.feedback.as_ref(), |d, (message, success)| {
                d.child(
                    div()
                        .id("order-feedback")
                        .mt_2()
                        .p_2()
                        .rounded_md()
                        .text_size(px(11.))
                        .line_height(relative(1.5))
                        .bg(rgb(if *success { 0xe9f7f4 } else { 0xffeff2 }))
                        .text_color(rgb(if *success { TEAL } else { RED }))
                        .child(message.clone())
                        .test_support(),
                )
            })
            .child(
                div()
                    .mt_2()
                    .text_size(px(10.))
                    .text_color(rgb(MUTED))
                    .child("仅本地演示 · 手动模拟成交 · 重启后重置账户"),
            )
            .into_any_element()
    }

    fn orders(&self, cx: &mut Context<Self>) -> AnyElement {
        let orders = cx.global::<DemoBroker>().orders.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(31.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(badge("模拟账户"))
                    .child(
                        div()
                            .text_color(rgb(MUTED))
                            .child(format!("{} 笔委托", orders.len())),
                    ),
            )
            .child(
                div()
                    .id("orders-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .child(div().min_w(px(620.)).child(table_header(&[
                        ("标的", 115.),
                        ("方向", 55.),
                        ("委托价", 90.),
                        ("数量", 65.),
                        ("状态", 105.),
                        ("操作", 150.),
                    ])))
                    .children(orders.into_iter().rev().map(|order| {
                        let id = order.id;
                        div()
                            .min_w(px(620.))
                            .h(px(35.))
                            .px_3()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0xf1f3f5))
                            .child(div().w(px(115.)).child(format!(
                                "{}  #{}",
                                order.symbol.code(),
                                order.id
                            )))
                            .child(
                                div()
                                    .w(px(55.))
                                    .text_color(rgb(order.side.color()))
                                    .child(order.side.label()),
                            )
                            .child(div().w(px(90.)).child(format!("{:.2}", order.price)))
                            .child(div().w(px(65.)).child(order.quantity.to_string()))
                            .child(
                                div()
                                    .w(px(105.))
                                    .text_color(rgb(if order.status == OrderStatus::Pending {
                                        0xe39e2c
                                    } else {
                                        MUTED
                                    }))
                                    .child(order.status.label()),
                            )
                            .when(order.status == OrderStatus::Pending, |d| {
                                d.child(
                                    div()
                                        .flex()
                                        .gap_1()
                                        .child(
                                            control(
                                                SharedString::from(format!("fill-{id}")),
                                                "模拟成交",
                                                true,
                                            )
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.update_global::<DemoBroker, _>(|broker, _| {
                                                    broker.fill(id)
                                                });
                                                cx.emit(ContentEvent::OrderSubmitted);
                                                cx.notify();
                                            }))
                                            .test_support(),
                                        )
                                        .child(
                                            control(
                                                SharedString::from(format!("cancel-{id}")),
                                                "撤单",
                                                false,
                                            )
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.update_global::<DemoBroker, _>(|broker, _| {
                                                    broker.cancel(id)
                                                });
                                                cx.emit(ContentEvent::OrderSubmitted);
                                                cx.notify();
                                            }))
                                            .test_support(),
                                        ),
                                )
                            })
                    })),
            )
            .into_any_element()
    }

    fn positions(&self, cx: &mut Context<Self>) -> AnyElement {
        let positions = cx.global::<DemoBroker>().positions.clone();
        let (usd, hkd) = {
            let broker = cx.global::<DemoBroker>();
            (broker.usd, broker.hkd)
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(36.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(badge("模拟持仓"))
                    .child(format!("现金 {usd:.2} USD"))
                    .child(div().text_color(rgb(MUTED)).child(format!("{hkd:.2} HKD"))),
            )
            .child(
                div()
                    .id("positions-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .child(div().min_w(px(605.)).child(table_header(&[
                        ("股票", 110.),
                        ("持有", 65.),
                        ("成本", 85.),
                        ("现价", 85.),
                        ("市值", 100.),
                        ("浮动盈亏", 130.),
                    ])))
                    .children(positions.into_iter().map(|position| {
                        let q = market::quote(position.symbol, self.tick);
                        let profit = (q.price - position.cost) * position.quantity as f64;
                        div()
                            .id(SharedString::from(format!(
                                "position-{}",
                                position.symbol.code()
                            )))
                            .min_w(px(605.))
                            .h(px(36.))
                            .px_3()
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(rgb(0xf1f3f5))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(0xf5fafb)))
                            .child(div().w(px(110.)).child(format!(
                                "{} {}",
                                position.symbol.code(),
                                position.symbol.name()
                            )))
                            .child(div().w(px(65.)).child(position.quantity.to_string()))
                            .child(div().w(px(85.)).child(format!("{:.2}", position.cost)))
                            .child(
                                div()
                                    .w(px(85.))
                                    .text_color(movement(q.change))
                                    .child(format!("{:.2}", q.price)),
                            )
                            .child(
                                div()
                                    .w(px(100.))
                                    .child(format!("{:.2}", q.price * position.quantity as f64)),
                            )
                            .child(
                                div()
                                    .w(px(130.))
                                    .text_color(movement(profit))
                                    .child(format!("{profit:+.2} {}", currency(position.symbol))),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_symbol(position.symbol, cx)
                            }))
                            .test_support()
                    })),
            )
            .into_any_element()
    }
}

impl Render for PanelContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(price) = self.pending_price.take() {
            self.price.update(cx, |input, cx| {
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
            .text_size(px(12.))
            .child(body)
    }
}

fn control(id: impl Into<ElementId>, text: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded(px(4.))
        .cursor_pointer()
        .bg(rgb(if active { 0xe8f7f6 } else { 0xffffff }))
        .text_color(rgb(if active { TEAL } else { INK }))
        .hover(|s| s.bg(rgb(0xeef4f7)))
        .child(text.into())
}
fn label(text: &'static str) -> Div {
    div()
        .mb_1()
        .text_size(px(11.))
        .text_color(rgb(MUTED))
        .child(text)
}
fn badge(text: &'static str) -> Div {
    div()
        .px_1()
        .py(px(2.))
        .rounded(px(3.))
        .bg(rgb(0xf0f5f8))
        .text_size(px(10.))
        .text_color(rgb(MUTED))
        .child(text)
}
fn movement(change: f64) -> Rgba {
    rgb(if change >= 0. { RED } else { TEAL })
}
fn currency(symbol: Symbol) -> &'static str {
    symbol.currency()
}
fn compact(value: f64) -> String {
    if value.abs() >= 1e12 {
        format!("{:.2} 万亿", value / 1e12)
    } else if value.abs() >= 1e8 {
        format!("{:.2} 亿", value / 1e8)
    } else if value.abs() >= 1e4 {
        format!("{:.2} 万", value / 1e4)
    } else {
        format!("{:.0}", value)
    }
}
fn metric(left: &'static str, left_value: String, right: &'static str, right_value: String) -> Div {
    metric_colored(left, left_value, rgb(INK), right, right_value, rgb(INK))
}
fn metric_colored(
    left: &'static str,
    left_value: String,
    left_color: Rgba,
    right: &'static str,
    right_value: String,
    right_color: Rgba,
) -> Div {
    div().h(px(20.)).flex().items_center().gap_2().children(
        [
            (left, left_value, left_color),
            (right, right_value, right_color),
        ]
        .into_iter()
        .map(|(label, value, color)| {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .justify_between()
                .gap_1()
                .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(label))
                .child(div().min_w_0().truncate().text_color(color).child(value))
        }),
    )
}
fn book_section(title: &'static str, color: u32) -> Div {
    div()
        .h(px(17.))
        .px_2()
        .flex()
        .items_center()
        .bg(rgb(0xf7f9fb))
        .text_size(px(10.))
        .text_color(rgb(color))
        .border_t_1()
        .border_color(rgb(LINE))
        .child(title)
}
fn dense_columns(columns: &[(&'static str, f32)], right: bool) -> Div {
    div()
        .h(px(20.))
        .px_2()
        .flex()
        .items_center()
        .flex_shrink_0()
        .bg(rgb(0xf5f7fa))
        .border_b_1()
        .border_color(rgb(LINE))
        .text_color(rgb(MUTED))
        .children(columns.iter().enumerate().map(|(index, (title, width))| {
            div()
                .w(px(*width))
                .flex_shrink_0()
                .when(right && index > 0, |d| d.text_right())
                .child(*title)
        }))
}
fn amount_cell(amount: f64, ratio: f32, color: u32, width: f32) -> Div {
    div()
        .relative()
        .w(px(width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_end()
        .px_1()
        .child(
            div()
                .absolute()
                .right_0()
                .top(px(2.))
                .bottom(px(2.))
                .w(relative(ratio.clamp(0., 1.)))
                .bg(rgb(color))
                .opacity(0.10),
        )
        .child(
            div()
                .relative()
                .text_color(rgb(color))
                .child(if amount == 0. {
                    "—".into()
                } else {
                    compact(amount)
                }),
        )
}
fn order_detail_table(
    id: &'static str,
    title: &'static str,
    orders: &[&market::MarketOrder],
    canceled: bool,
) -> Div {
    let max = orders.iter().map(|order| order.amount).fold(1., f64::max);
    div()
        .h_full()
        .flex_1()
        .min_w(px(480.))
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(rgb(LINE))
        .child(
            div()
                .h(px(22.))
                .px_2()
                .flex()
                .items_center()
                .justify_between()
                .flex_shrink_0()
                .bg(rgb(0xf2f6fa))
                .child(div().font_weight(FontWeight::MEDIUM).child(title))
                .child(
                    div()
                        .text_color(rgb(MUTED))
                        .child(format!("{} 笔", orders.len())),
                ),
        )
        .child(dense_columns(
            &[
                (
                    if canceled {
                        "撤单时间"
                    } else {
                        "委托时间"
                    },
                    65.,
                ),
                ("方向", 40.),
                ("委托价格", 70.),
                ("股数", 60.),
                ("委托金额", 85.),
                (if canceled { "委托时间" } else { "状态" }, 90.),
            ],
            false,
        ))
        .child(
            div()
                .id(id)
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .when(orders.is_empty(), |d| {
                    d.child(
                        div()
                            .p_3()
                            .text_color(rgb(MUTED))
                            .child("没有满足条件的委托"),
                    )
                })
                .children(orders.iter().map(|order| {
                    let color = if order.buy { RED } else { TEAL };
                    div()
                        .h(px(20.))
                        .px_2()
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(0xf0f3f6))
                        .child(
                            div()
                                .w(px(65.))
                                .flex_shrink_0()
                                .text_color(rgb(MUTED))
                                .child(order.canceled_at.as_ref().unwrap_or(&order.time).clone()),
                        )
                        .child(
                            div()
                                .w(px(40.))
                                .flex_shrink_0()
                                .text_color(rgb(color))
                                .child(match (canceled, order.buy) {
                                    (true, true) => "撤买",
                                    (true, false) => "撤卖",
                                    (false, true) => "委买",
                                    (false, false) => "委卖",
                                }),
                        )
                        .child(
                            div()
                                .w(px(70.))
                                .flex_shrink_0()
                                .text_color(rgb(color))
                                .child(format!("{:.3}", order.price)),
                        )
                        .child(
                            div()
                                .w(px(60.))
                                .flex_shrink_0()
                                .text_color(rgb(color))
                                .child(order.size.to_string()),
                        )
                        .child(amount_cell(
                            order.amount,
                            (order.amount / max) as f32,
                            color,
                            85.,
                        ))
                        .child(
                            div()
                                .w(px(90.))
                                .flex_shrink_0()
                                .px_2()
                                .text_color(rgb(MUTED))
                                .child(if canceled {
                                    order.time.clone()
                                } else {
                                    "排队中".into()
                                }),
                        )
                })),
        )
}
fn book_row(label: String, price: f64, quantity: u64, max: f32, color: u32) -> Div {
    div()
        .relative()
        .h(px(20.))
        .mx_2()
        .flex()
        .items_center()
        .child(
            div()
                .absolute()
                .right_0()
                .top_0()
                .bottom_0()
                .w(relative(quantity as f32 / max * 0.65))
                .bg(rgb(color))
                .opacity(0.07),
        )
        .child(div().w(px(36.)).text_color(rgb(color)).child(label))
        .child(
            div()
                .flex_1()
                .text_right()
                .text_color(rgb(color))
                .child(format!("{price:.3}")),
        )
        .child(div().w(px(90.)).text_right().child(quantity.to_string()))
}
fn table_header(columns: &[(&'static str, f32)]) -> Div {
    div()
        .h(px(27.))
        .px_3()
        .flex()
        .items_center()
        .bg(rgb(0xf8fafc))
        .text_color(rgb(MUTED))
        .children(
            columns
                .iter()
                .map(|(title, width)| div().w(px(*width)).flex_shrink_0().child(*title)),
        )
}

/// Exercise actual input, submit, cancellation and fill controls in isolated
/// native windows. The live application's broker is restored after this check.
#[cfg(feature = "test-support")]
pub(crate) fn smoke_demo_orders(cx: &mut HeadlessAppContext) -> anyhow::Result<()> {
    use anyhow::ensure;
    use gpui_kit::test::TestWindowExt;

    let previous = cx.update(|cx| {
        let previous = cx.global::<DemoBroker>().clone();
        cx.set_global(DemoBroker::default());
        previous
    });
    let mut handles = Vec::new();
    let result = (|| -> anyhow::Result<()> {
        let mut open = |kind| -> anyhow::Result<(AnyWindowHandle, Entity<PanelContent>)> {
            let result = cx.update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(50.), px(50.)),
                            size(px(800.), px(440.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        cx.new(|cx| {
                            PanelContent::new(kind, Symbol::Aapl, Period::Intraday, window, cx)
                        })
                    },
                )
            })?;
            handles.push(result.0);
            Ok(result)
        };
        let (entry, entry_view) = open(PanelKind::OrderEntry)?;
        let (orders, _) = open(PanelKind::Orders)?;
        let (positions, _) = open(PanelKind::Positions)?;
        for handle in &handles {
            cx.update_window(*handle, |_, window, cx| window.render_frame(cx))?;
        }

        // Invalid quantity must produce visible feedback, with no new order.
        cx.update_window(entry, |_, window, cx| {
            window.click("order-price", cx);
            window.press("ctrl-a", cx);
            window.input("340.00", cx);
            window.click("order-quantity", cx);
            window.press("ctrl-a", cx);
            window.input("0", cx);
            window.click("submit-order", cx);
            ensure!(
                window.find("order-feedback").visible(),
                "Invalid quantity has no visible feedback"
            );
            ensure!(
                cx.global::<DemoBroker>().orders.len() == 1,
                "Invalid quantity created an order"
            );
            ensure!(
                entry_view
                    .read(cx)
                    .feedback
                    .as_ref()
                    .is_some_and(|(_, valid)| !valid),
                "Invalid quantity was accepted"
            );
            window.click("order-quantity", cx);
            window.press("ctrl-a", cx);
            window.input("10", cx);
            window.click("submit-order", cx);
            ensure!(
                cx.global::<DemoBroker>().orders.len() == 2,
                "Submit did not record a pending order"
            );
            ensure!(
                cx.global::<DemoBroker>().available_cash(Symbol::Aapl) == 96_600.,
                "Pending order did not reserve cash"
            );
            Ok::<_, anyhow::Error>(())
        })??;

        cx.update_window(orders, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            window.click("cancel-1002", cx);
            ensure!(
                cx.global::<DemoBroker>().orders[1].status == OrderStatus::Cancelled,
                "Cancel button did not cancel the order"
            );
            ensure!(
                cx.global::<DemoBroker>().available_cash(Symbol::Aapl) == 100_000.,
                "Cancellation did not release reserved cash"
            );
            Ok::<_, anyhow::Error>(())
        })??;

        cx.update_window(entry, |_, window, cx| window.click("submit-order", cx))?;
        cx.update_window(orders, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            window.click("fill-1003", cx);
            ensure!(
                cx.global::<DemoBroker>().available_shares(Symbol::Aapl) == 110,
                "Buy fill did not update the holding"
            );
            ensure!(
                cx.global::<DemoBroker>().usd == 96_600.,
                "Buy fill did not debit cash"
            );
            Ok::<_, anyhow::Error>(())
        })??;

        cx.update_window(positions, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            ensure!(
                window.find("position-AAPL").visible(),
                "Updated AAPL holding is not visible"
            );
            Ok::<_, anyhow::Error>(())
        })??;

        cx.update_window(entry, |_, window, cx| {
            window.click("order-sell", cx);
            window.click("submit-order", cx);
        })?;
        cx.update_window(orders, |_, window, cx| {
            window.refresh();
            window.render_frame(cx);
            window.click("fill-1004", cx);
            ensure!(
                cx.global::<DemoBroker>().available_shares(Symbol::Aapl) == 100,
                "Sell fill did not reduce the holding"
            );
            ensure!(
                cx.global::<DemoBroker>().usd == 100_000.,
                "Sell fill did not credit cash"
            );
            Ok::<_, anyhow::Error>(())
        })??;
        println!(
            "PASS: native order input validation, pending buy, cancel, buy fill, sell fill and shared positions/cash"
        );
        Ok(())
    })();
    for handle in handles {
        let _ = cx.update_window(handle, |_, window, _| window.remove_window());
    }
    cx.update(|cx| cx.set_global(previous));
    result
}

/// Exercise the reference panel controls through real GPUI input events.
#[cfg(feature = "test-support")]
pub(crate) fn smoke_market_panels(cx: &mut HeadlessAppContext) -> anyhow::Result<()> {
    use anyhow::ensure;
    use gpui_kit::test::TestWindowExt;
    let mut handles = Vec::new();
    let result = (|| -> anyhow::Result<()> {
        let mut open = |kind| -> anyhow::Result<(AnyWindowHandle, Entity<PanelContent>)> {
            let result = cx.update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                            point(px(50.), px(50.)),
                            size(px(1100.), px(560.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |window, cx| {
                        cx.new(|cx| {
                            PanelContent::new(kind, Symbol::Aapl, Period::Intraday, window, cx)
                        })
                    },
                )
            })?;
            handles.push(result.0);
            Ok(result)
        };
        let (details, details_view) = open(PanelKind::OrderDetails)?;
        let (flow, flow_view) = open(PanelKind::TradeDetails)?;
        let (book, book_view) = open(PanelKind::OrderBook)?;
        for handle in &handles {
            cx.update_window(*handle, |_, window, cx| window.render_frame(cx))?;
        }
        cx.update_window(details, |_, window, cx| {
            ensure!(
                window.find("market-min-amount").visible(),
                "Market amount filter is not visible"
            );
            window.click("market-min-amount", cx);
            window.input("5", cx);
            window.click("market-max-amount", cx);
            window.input("20", cx);
            window.click("market-min-quantity", cx);
            window.input("100", cx);
            window.click("market-direction-1", cx);
            window.click("market-filter-enabled", cx);
            let snapshot = details_view.read(cx).snapshot(cx);
            ensure!(
                snapshot["order_filter"]["enabled"] == true,
                "Market filter did not toggle"
            );
            ensure!(
                snapshot["order_filter"]["min_amount"] == "5",
                "Market amount input not saved"
            );
            ensure!(
                snapshot["order_filter"]["max_amount"] == "20",
                "Market amount upper bound not saved"
            );
            ensure!(
                snapshot["order_filter"]["min_quantity"] == "100",
                "Market quantity input not saved"
            );
            ensure!(
                snapshot["order_filter"]["direction"] == 1,
                "Market direction did not change"
            );
            // Rebuilding the content, as a cross-window move does, must keep the filters.
            let restored = cx.new(|cx| {
                PanelContent::new(
                    PanelKind::OrderDetails,
                    Symbol::Aapl,
                    Period::Intraday,
                    window,
                    cx,
                )
            });
            restored.update(cx, |view, cx| view.restore(&snapshot, window, cx));
            ensure!(
                restored.read(cx).snapshot(cx) == snapshot,
                "Market filter snapshot failed to restore"
            );
            Ok::<_, anyhow::Error>(())
        })??;
        cx.update_window(flow, |_, window, cx| {
            window.click("flow-large", cx);
            ensure!(
                flow_view.read(cx).snapshot(cx)["large_trades_only"] == true,
                "Large execution filter did not change"
            );
            window.click("flow-all", cx);
            ensure!(
                flow_view.read(cx).snapshot(cx)["large_trades_only"] == false,
                "Execution filter did not reset"
            );
            Ok::<_, anyhow::Error>(())
        })??;
        cx.update_window(book, |_, window, cx| {
            window.click("book-queue", cx);
            ensure!(
                book_view.read(cx).snapshot(cx)["book_queue"] == true,
                "Book queue toggle did not change"
            );
            window.click("book-depth", cx);
            ensure!(
                book_view.read(cx).snapshot(cx)["book_queue"] == false,
                "Book depth toggle did not reset"
            );
            Ok::<_, anyhow::Error>(())
        })??;
        println!(
            "PASS: market order amount/quantity/direction filters, restored filter state, large execution filter and order-book queue controls"
        );
        Ok(())
    })();
    for handle in handles {
        let _ = cx.update_window(handle, |_, window, _| window.remove_window());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{ContentSnapshot, DemoBroker, OrderFilterSnapshot, OrderRange, OrderStatus, Side};
    use crate::market::Symbol;

    #[test]
    fn pending_buys_reserve_cash_and_cancellation_releases_it() {
        let mut broker = DemoBroker::default();
        let id = broker.submit(Symbol::Msft, Side::Buy, 500., 100).unwrap();
        assert_eq!(broker.available_cash(Symbol::Aapl), 50_000.);
        assert!(broker.submit(Symbol::Aapl, Side::Buy, 600., 100).is_err());
        assert_eq!(broker.available_cash(Symbol::Tencent), 500_000.);
        broker.cancel(id);
        assert_eq!(broker.available_cash(Symbol::Msft), 100_000.);
    }

    #[test]
    fn simulated_fills_update_positions_and_do_not_fill_twice() {
        let mut broker = DemoBroker::default();
        let id = broker.submit(Symbol::Aapl, Side::Sell, 340., 60).unwrap();
        assert_eq!(broker.available_shares(Symbol::Aapl), 40);
        assert!(broker.submit(Symbol::Aapl, Side::Sell, 340., 50).is_err());
        broker.fill(id);
        broker.fill(id);
        assert_eq!(broker.available_shares(Symbol::Aapl), 40);
        assert_eq!(broker.usd, 120_400.);
        assert!(broker.orders.iter().find(|o| o.id == id).unwrap().status == OrderStatus::Filled);
        assert!(broker.submit(Symbol::Aapl, Side::Buy, f64::NAN, 1).is_err());
    }
    #[test]
    fn legacy_content_snapshots_keep_their_values_and_default_new_filters() {
        let saved: ContentSnapshot = serde_json::from_value(serde_json::json!({
            "query": "AAPL", "price": "340.00", "quantity": "250", "side": "Sell",
            "sort": 2, "chart": {"bars_visible": 70, "offset_from_latest": 9}
        }))
        .unwrap();
        assert_eq!(saved.price, "340.00");
        assert_eq!(saved.quantity, "250");
        assert_eq!(saved.chart.bars_visible, 70);
        assert_eq!(saved.chart.offset_from_latest, 9);
        assert!(!saved.order_filter.enabled);
        assert!(saved.order_filter.min_amount.is_empty());
        assert!(!saved.large_trades_only && !saved.book_queue);
        assert_eq!(saved.members, Symbol::ALL);
    }

    #[test]
    fn stock_members_preserve_explicit_empty_lists_and_remove_duplicate_symbols() {
        let mut value = serde_json::json!({
            "query": "", "price": "340.00", "quantity": "100", "side": "Buy",
            "sort": 0, "chart": {"bars_visible": 70, "offset_from_latest": 0},
            "members": []
        });
        let empty: ContentSnapshot = serde_json::from_value(value.clone()).unwrap();
        assert!(empty.members.is_empty());
        let restored: ContentSnapshot =
            serde_json::from_value(serde_json::to_value(empty).unwrap()).unwrap();
        assert!(restored.members.is_empty());

        value["members"] = serde_json::json!(["Tencent", "Msft", "Tencent", "Msft"]);
        let saved: ContentSnapshot = serde_json::from_value(value).unwrap();
        assert_eq!(saved.members, [Symbol::Tencent, Symbol::Msft]);
    }

    #[test]
    fn market_order_filters_apply_currency_units_and_inclusive_boundaries() {
        let mut order = crate::market::market_orders(Symbol::Aapl, 0).remove(0);
        order.amount = 200_000.;
        order.size = 100;
        let mut filter = OrderFilterSnapshot {
            min_amount: "20".into(),
            max_amount: "20".into(),
            min_quantity: "100".into(),
            max_quantity: "100".into(),
            ..Default::default()
        };
        assert!(OrderRange::parse(&filter).unwrap().contains(&order));
        order.amount += 0.01;
        assert!(!OrderRange::parse(&filter).unwrap().contains(&order));
        order.amount = 200_000.;
        order.size = 101;
        assert!(!OrderRange::parse(&filter).unwrap().contains(&order));
        filter.min_amount = "21".into();
        assert!(OrderRange::parse(&filter).is_err());
        for invalid in ["NaN", "inf", "-1", "abc", "1e308"] {
            filter.min_amount = invalid.into();
            assert!(
                OrderRange::parse(&filter).is_err(),
                "Accepted invalid filter {invalid}"
            );
        }
        assert!(
            OrderRange::parse(&OrderFilterSnapshot::default())
                .unwrap()
                .contains(&order)
        );
    }
}
