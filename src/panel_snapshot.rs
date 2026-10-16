//! Durable panel content, independent of native views and disk I/O.
//!
//! Null is the legacy/new-panel marker for absent content. A present snapshot
//! must decode completely before any live input, chart or window is changed.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::market::Symbol;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChartViewport {
    pub bars_visible: usize,
    pub offset_from_latest: usize,
}

impl Default for ChartViewport {
    fn default() -> Self {
        Self {
            bars_visible: 240,
            offset_from_latest: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) enum Side {
    Buy,
    Sell,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct OrderFilterSnapshot {
    pub enabled: bool,
    pub min_amount: String,
    pub max_amount: String,
    pub min_quantity: String,
    pub max_quantity: String,
    pub direction: u8,
}

/// Keep the existing JSON field names and legacy defaults. Even fields unused
/// by a particular view are retained when a legacy snapshot is saved again.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ContentSnapshot {
    pub query: String,
    pub price: String,
    pub quantity: String,
    pub side: Side,
    pub sort: u8,
    pub chart: ChartViewport,
    #[serde(default)]
    pub order_filter: OrderFilterSnapshot,
    #[serde(default)]
    pub large_trades_only: bool,
    #[serde(default)]
    pub book_queue: bool,
    #[serde(default = "default_members", deserialize_with = "read_members")]
    pub members: Vec<Symbol>,
}

fn default_members() -> Vec<Symbol> {
    Symbol::LEGACY_DEFAULTS.to_vec()
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

impl ContentSnapshot {
    pub fn new(price: String) -> Self {
        Self {
            query: String::new(),
            price,
            quantity: "100".into(),
            side: Side::Buy,
            sort: 0,
            chart: ChartViewport::default(),
            order_filter: OrderFilterSnapshot::default(),
            large_trades_only: false,
            book_queue: false,
            members: Symbol::ALL.to_vec(),
        }
    }

    pub fn decode(value: &serde_json::Value) -> Result<Option<Self>> {
        if value.is_null() {
            return Ok(None);
        }
        let mut saved: Self =
            serde_json::from_value(value.clone()).context("Invalid known panel content")?;
        // Input drafts may contain invalid numeric text; that is legitimate UI
        // state. Validate shape and selectors here, not submission rules.
        // These selectors were clamped by every earlier restore path. Retain
        // that normalization instead of rejecting a previously readable file.
        saved.sort = saved.sort.min(2);
        saved.order_filter.direction = saved.order_filter.direction.min(2);
        Ok(Some(saved))
    }
}

pub fn validate(value: &serde_json::Value) -> Result<()> {
    ContentSnapshot::decode(value).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_content_and_legacy_defaults_remain_distinct_from_corruption() {
        assert!(
            ContentSnapshot::decode(&serde_json::Value::Null)
                .unwrap()
                .is_none()
        );
        let legacy = serde_json::json!({
            "query": "AAPL", "price": "unfinished", "quantity": "", "side": "Sell",
            "sort": 2, "chart": { "bars_visible": 70, "offset_from_latest": 9 }
        });
        let saved = ContentSnapshot::decode(&legacy).unwrap().unwrap();
        assert_eq!(saved.price, "unfinished");
        assert_eq!(saved.members, Symbol::LEGACY_DEFAULTS);
        assert!(!saved.order_filter.enabled);
        for bad in [
            serde_json::json!({}),
            serde_json::json!("broken"),
            serde_json::json!([1, 2]),
        ] {
            assert!(ContentSnapshot::decode(&bad).is_err());
        }
        let mut bad = legacy.clone();
        bad["chart"]["bars_visible"] = serde_json::json!("broken");
        assert!(ContentSnapshot::decode(&bad).is_err());
        let mut bad = legacy;
        bad["side"] = serde_json::json!("unknown");
        assert!(ContentSnapshot::decode(&bad).is_err());
    }

    #[test]
    fn legacy_selector_normalization_is_preserved() {
        let mut value = serde_json::to_value(ContentSnapshot::new("10".into())).unwrap();
        value["sort"] = serde_json::json!(200);
        value["order_filter"]["direction"] = serde_json::json!(200);
        let saved = ContentSnapshot::decode(&value).unwrap().unwrap();
        assert_eq!(saved.sort, 2);
        assert_eq!(saved.order_filter.direction, 2);
    }
}
