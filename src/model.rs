use crate::market::{Period, Symbol};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PanelKind {
    Watchlist,
    Chart,
    Quote,
    OrderBook,
    Trades,
    TradeDetails,
    OrderDetails,
    News,
    OrderEntry,
    Orders,
    Positions,
}
impl PanelKind {
    pub const ALL: [Self; 11] = [
        Self::Watchlist,
        Self::Chart,
        Self::Quote,
        Self::OrderBook,
        Self::Trades,
        Self::TradeDetails,
        Self::OrderDetails,
        Self::News,
        Self::OrderEntry,
        Self::Orders,
        Self::Positions,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Watchlist => "自选列表",
            Self::Chart => "行情图表",
            Self::Quote => "关键数据",
            Self::OrderBook => "买卖十档",
            Self::Trades => "成交明细",
            Self::TradeDetails => "逐笔成交",
            Self::OrderDetails => "委托明细",
            Self::News => "资讯公告",
            Self::OrderEntry => "模拟交易",
            Self::Orders => "当日订单",
            Self::Positions => "模拟持仓",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PanelModel {
    pub id: u64,
    pub kind: PanelKind,
    pub symbol: Symbol,
    pub period: Period,
    pub link_group: u8,
}
impl PanelModel {
    pub fn new(id: u64, kind: PanelKind) -> Self {
        Self {
            id,
            kind,
            symbol: Symbol::Aapl,
            period: Period::Intraday,
            link_group: 1,
        }
    }
}
