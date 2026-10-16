//! Durable panel content, independent of native views and disk I/O.
//!
//! Null is the legacy/new-panel marker for absent content. A present snapshot
//! must decode completely before any live input, chart or window is changed.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::market::Symbol;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ChartViewport {
    pub(crate) bars_visible: usize,
    pub(crate) offset_from_latest: usize,
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
    pub(crate) enabled: bool,
    pub(crate) min_amount: String,
    pub(crate) max_amount: String,
    pub(crate) min_quantity: String,
    pub(crate) max_quantity: String,
    pub(crate) direction: u8,
}

/// Keep the existing JSON field names and legacy defaults. Even fields unused
/// by a particular view are retained when a legacy snapshot is saved again.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct ContentSnapshot {
    pub(crate) query: String,
    pub(crate) price: String,
    pub(crate) quantity: String,
    pub(crate) side: Side,
    pub(crate) sort: u8,
    pub(crate) chart: ChartViewport,
    #[serde(default)]
    pub(crate) order_filter: OrderFilterSnapshot,
    #[serde(default)]
    pub(crate) large_trades_only: bool,
    #[serde(default)]
    pub(crate) book_queue: bool,
    #[serde(default = "default_members", deserialize_with = "read_members")]
    pub(crate) members: Vec<Symbol>,
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
    pub(crate) fn new(price: String) -> Self {
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

    pub(crate) fn decode(value: &serde_json::Value) -> Result<Option<Self>> {
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

pub(crate) fn validate(value: &serde_json::Value) -> Result<()> {
    ContentSnapshot::decode(value).map(|_| ())
}

#[cfg(test)]
mod tests;
