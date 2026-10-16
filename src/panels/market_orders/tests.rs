use super::{OrderFilterSnapshot, OrderRange, Symbol};

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
