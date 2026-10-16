use super::*;

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
fn shanghai_and_shenzhen_share_cny_without_spending_other_currencies() {
    let mut broker = DemoBroker::default();
    let id = broker
        .submit(Symbol::Yinli, Side::Buy, 20., 10_000)
        .unwrap();
    assert_eq!(broker.available_cash(Symbol::Yangguang), 300_000.);
    assert!(
        broker
            .submit(Symbol::Yangguang, Side::Buy, 15., 30_000)
            .is_err()
    );
    assert_eq!(broker.available_cash(Symbol::Aapl), 100_000.);
    assert_eq!(broker.available_cash(Symbol::Tencent), 500_000.);
    broker.fill(id);
    broker.fill(id);
    assert_eq!(broker.cny, 300_000.);
    assert_eq!(broker.available_shares(Symbol::Yinli), 10_000);
    let sell = broker.submit(Symbol::Yinli, Side::Sell, 21., 100).unwrap();
    broker.fill(sell);
    assert_eq!(broker.cny, 302_100.);
    assert_eq!(broker.usd, 100_000.);
    assert_eq!(broker.hkd, 500_000.);
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
