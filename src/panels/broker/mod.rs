//! Process-local simulated account; no network or disk side effects.
use super::{App, Global, Side, Symbol};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
pub(super) enum OrderStatus {
    Pending,
    Filled,
    Cancelled,
}
impl OrderStatus {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Pending => "待模拟成交",
            Self::Filled => "模拟已成交",
            Self::Cancelled => "已撤销",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct DemoOrder {
    pub(super) id: u64,
    pub(super) symbol: Symbol,
    pub(super) side: Side,
    pub(super) price: f64,
    pub(super) quantity: u64,
    pub(super) status: OrderStatus,
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct DemoPosition {
    pub(super) symbol: Symbol,
    pub(super) quantity: u64,
    pub(super) cost: f64,
}

/// Session-only accounting. Demonstration balances are independent by currency.
#[derive(Clone)]
pub(super) struct DemoBroker {
    pub(super) orders: Vec<DemoOrder>,
    pub(super) positions: Vec<DemoPosition>,
    pub(super) usd: f64,
    pub(super) hkd: f64,
    pub(super) cny: f64,
    next_id: u64,
}
impl Global for DemoBroker {}

/// The composition root owns process services. Constructing a view must not
/// create or reset a shared account as a side effect.
pub(crate) fn init_broker(cx: &mut App) {
    if !cx.has_global::<DemoBroker>() {
        cx.set_global(DemoBroker::default());
    }
}

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
            cny: 500_000.,
            next_id: 1002,
        }
    }
}
impl DemoBroker {
    pub(super) fn available_cash(&self, symbol: Symbol) -> f64 {
        let balance = match symbol.currency() {
            "HKD" => self.hkd,
            "CNY" => self.cny,
            _ => self.usd,
        };
        balance
            - self
                .orders
                .iter()
                .filter(|o| {
                    o.symbol.currency() == symbol.currency()
                        && o.side == Side::Buy
                        && o.status == OrderStatus::Pending
                })
                .map(|o| o.price * o.quantity as f64)
                .sum::<f64>()
    }
    pub(super) fn available_shares(&self, symbol: Symbol) -> u64 {
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
    pub(super) fn submit(
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
    pub(super) fn cancel(&mut self, id: u64) {
        if let Some(order) = self
            .orders
            .iter_mut()
            .find(|o| o.id == id && o.status == OrderStatus::Pending)
        {
            order.status = OrderStatus::Cancelled;
        }
    }
    pub(super) fn fill(&mut self, id: u64) {
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
        let balance = match order.symbol.currency() {
            "HKD" => &mut self.hkd,
            "CNY" => &mut self.cny,
            _ => &mut self.usd,
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

#[cfg(test)]
mod tests;
