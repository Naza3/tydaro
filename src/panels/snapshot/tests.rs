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
    assert_eq!(saved.members, Symbol::LEGACY_DEFAULTS);
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
