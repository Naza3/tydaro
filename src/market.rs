//! Deterministic local market data shared by every panel.
//!
//! `tick` advances the demo by one simulated minute, starting at 12:29. The
//! simulated session restarts after 16:00; no clock, network or random source is
//! involved. Prices remain integer cents until converted for presentation.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Symbol {
    Aapl,
    Msft,
    Nvda,
    Tsla,
    Tencent,
    Alibaba,
}

impl Symbol {
    pub const ALL: [Self; 6] = [
        Self::Aapl,
        Self::Msft,
        Self::Nvda,
        Self::Tsla,
        Self::Tencent,
        Self::Alibaba,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Self::Aapl => "AAPL",
            Self::Msft => "MSFT",
            Self::Nvda => "NVDA",
            Self::Tsla => "TSLA",
            Self::Tencent => "00700",
            Self::Alibaba => "09988",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Aapl => "苹果",
            Self::Msft => "微软",
            Self::Nvda => "英伟达",
            Self::Tsla => "特斯拉",
            Self::Tencent => "腾讯控股",
            Self::Alibaba => "阿里巴巴",
        }
    }

    pub fn market(self) -> &'static str {
        match self {
            Self::Tencent | Self::Alibaba => "HK",
            _ => "US",
        }
    }

    fn seed(self) -> u64 {
        match self {
            Self::Aapl => 11,
            Self::Msft => 29,
            Self::Nvda => 47,
            Self::Tsla => 71,
            Self::Tencent => 97,
            Self::Alibaba => 131,
        }
    }

    fn previous_close(self) -> i64 {
        match self {
            Self::Aapl => 34_097,
            Self::Msft => 51_246,
            Self::Nvda => 18_724,
            Self::Tsla => 43_812,
            Self::Tencent => 64_500,
            Self::Alibaba => 17_160,
        }
    }

    fn drift(self) -> i64 {
        match self {
            Self::Aapl => -72,
            Self::Msft => 238,
            Self::Nvda => 324,
            Self::Tsla => -619,
            Self::Tencent => 520,
            Self::Alibaba => -164,
        }
    }

    // Illustrative fundamentals for the local demo, not reported company data.
    fn float_shares(self) -> u64 {
        self.shares()
            * match self {
                Self::Aapl => 97,
                Self::Msft => 96,
                Self::Nvda => 95,
                Self::Tsla => 87,
                Self::Tencent => 89,
                Self::Alibaba => 92,
            }
            / 100
    }

    fn earnings_per_share(self) -> f64 {
        cents(self.previous_close())
            / match self {
                Self::Aapl => 38.63,
                Self::Msft => 36.42,
                Self::Nvda => 49.12,
                Self::Tsla => 109.30,
                Self::Tencent => 24.60,
                Self::Alibaba => 20.85,
            }
    }

    fn shares(self) -> u64 {
        match self {
            Self::Aapl => 14_594_000_000,
            Self::Msft => 7_432_000_000,
            Self::Nvda => 24_300_000_000,
            Self::Tsla => 3_220_000_000,
            Self::Tencent => 9_150_000_000,
            Self::Alibaba => 18_560_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Period {
    Intraday,
    Day,
    Week,
    Month,
    Minute1,
    Minute5,
}

impl Period {
    pub const ALL: [Self; 6] = [
        Self::Intraday,
        Self::Day,
        Self::Week,
        Self::Month,
        Self::Minute1,
        Self::Minute5,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Intraday => "分时",
            Self::Day => "日 K",
            Self::Week => "周 K",
            Self::Month => "月 K",
            Self::Minute1 => "1 分",
            Self::Minute5 => "5 分",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quote {
    pub price: f64,
    pub change: f64,
    pub change_pct: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub previous_close: f64,
    pub volume: u64,
    pub turnover: f64,
    pub market_cap: f64,
    pub float_market_cap: f64,
    pub total_shares: u64,
    pub float_shares: u64,
    /// Session volume / illustrative free-float shares, in percent.
    pub turnover_rate: f64,
    /// Session volume / the demo's expected volume for the elapsed minutes.
    pub volume_ratio: f64,
    pub buy_volume: u64,
    pub sell_volume: u64,
    /// Based on an illustrative EPS fixture; no real fundamentals are fetched.
    pub pe_ttm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub time: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Trade {
    pub time: String,
    pub price: f64,
    pub size: u64,
    pub buy: bool,
    /// Number of executions aggregated into this three-second summary row.
    pub count: u64,
}

/// One execution underlying the aggregated成交明细 rows.
#[derive(Clone, Debug, PartialEq)]
pub struct Execution {
    pub time: String,
    pub price: f64,
    pub size: u64,
    pub amount: f64,
    pub buy: bool,
    pub buy_order_amount: f64,
    pub sell_order_amount: f64,
}

/// Simulated public market order, separate from the local trading account.
/// `time` is the submission time; canceled orders retain their original price
/// and size and carry a later `canceled_at`. Pending orders sum to the book.
#[derive(Clone, Debug, PartialEq)]
pub struct MarketOrder {
    pub id: u64,
    pub time: String,
    pub price: f64,
    pub size: u64,
    pub amount: f64,
    pub buy: bool,
    pub canceled_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Level {
    pub price: f64,
    pub size: u64,
}

#[derive(Clone, Copy)]
struct Candle {
    open: i64,
    high: i64,
    low: i64,
    close: i64,
    volume: u64,
}

const BAR_COUNT: u64 = 180;

fn cents(value: i64) -> f64 {
    value as f64 / 100.0
}

// A bounded integer hash. Wrapping arithmetic is intentional and makes the
// sample independent of debug/release overflow settings.
fn noise(seed: u64, index: u64) -> u64 {
    let mut value = seed.wrapping_add(index.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

// Interpolate deterministic anchor points to give charts broad trends and
// short movements without unrelated random jumps on each render.
fn wave(seed: u64, index: u64, width: u64, amplitude: i64) -> i64 {
    let bucket = index / width;
    let offset = (index % width) as i64;
    let anchor = |position| (noise(seed, position) % (amplitude as u64 * 2 + 1)) as i64 - amplitude;
    let left = anchor(bucket);
    left + (anchor(bucket + 1) - left) * offset / width as i64
}

fn minute(tick: u64) -> u64 {
    // Keep all snapshots inside a 390-minute demonstration session. Hong Kong
    // symbols use the same synthetic clock; this is not an exchange calendar.
    BAR_COUNT - 1 + tick % (391 - BAR_COUNT)
}

fn minute_close(symbol: Symbol, index: u64) -> i64 {
    let base = symbol.previous_close();
    let amplitude = base / 250;
    base + symbol.drift() * (index.min(180) as i64 + 180) / 360
        + wave(symbol.seed(), index, 31, amplitude)
        + wave(symbol.seed() + 7, index, 7, amplitude / 3)
        + (noise(symbol.seed(), index) % 13) as i64
        - 6
}

fn minute_candle(symbol: Symbol, index: u64) -> Candle {
    let close = minute_close(symbol, index);
    let open = if index == 0 {
        symbol.previous_close() + symbol.drift() / 3
    } else {
        minute_close(symbol, index - 1)
    };
    let wick = 2 + (noise(symbol.seed() + 17, index) % 16) as i64;
    Candle {
        open,
        high: open.max(close) + wick,
        low: open.min(close) - wick,
        close,
        volume: 18_000 + noise(symbol.seed() + 23, index) % 110_000,
    }
}

fn public_bar(time: String, candle: Candle) -> Bar {
    Bar {
        time,
        open: cents(candle.open),
        high: cents(candle.high),
        low: cents(candle.low),
        close: cents(candle.close),
        volume: candle.volume,
    }
}

fn minute_label(index: u64) -> String {
    let wall_minutes = 9 * 60 + 30 + index;
    format!("{:02}:{:02}", wall_minutes / 60, wall_minutes % 60)
}

/// Snapshot statistics covering the simulated session up to `tick`.
pub fn quote(symbol: Symbol, tick: u64) -> Quote {
    let last_minute = minute(tick);
    let current = minute_close(symbol, last_minute);
    let opening = minute_candle(symbol, 0).open;
    let mut high = opening;
    let mut low = opening;
    let mut volume = 0;
    let mut buy_volume = 0;
    let mut turnover_cents = 0_u64;
    for index in 0..=last_minute {
        let candle = minute_candle(symbol, index);
        high = high.max(candle.high);
        low = low.min(candle.low);
        volume += candle.volume;
        let buy_percent = if candle.close >= candle.open { 61 } else { 39 };
        buy_volume += candle.volume * buy_percent / 100;
        let average = (candle.open + candle.high + candle.low + candle.close) / 4;
        turnover_cents += average as u64 * candle.volume;
    }
    Quote {
        price: cents(current),
        change: cents(current - symbol.previous_close()),
        change_pct: (current - symbol.previous_close()) as f64 / symbol.previous_close() as f64
            * 100.0,
        open: cents(opening),
        high: cents(high),
        low: cents(low),
        previous_close: cents(symbol.previous_close()),
        volume,
        turnover: turnover_cents as f64 / 100.0,
        market_cap: current as f64 * symbol.shares() as f64 / 100.0,
        float_market_cap: current as f64 * symbol.float_shares() as f64 / 100.0,
        total_shares: symbol.shares(),
        float_shares: symbol.float_shares(),
        turnover_rate: volume as f64 / symbol.float_shares() as f64 * 100.0,
        volume_ratio: volume as f64 / ((last_minute + 1) * (58_000 + symbol.seed() * 101)) as f64,
        buy_volume,
        sell_volume: volume - buy_volume,
        pe_ttm: cents(current) / symbol.earnings_per_share(),
    }
}

fn historical_close(symbol: Symbol, age: u64, period: Period) -> i64 {
    let base = symbol.previous_close();
    let scale = match period {
        Period::Month => 12,
        Period::Week => 3,
        _ => 1,
    };
    let movement =
        |at| wave(symbol.seed(), at, 23, base / 15) + wave(symbol.seed() + 41, at, 5, base / 50);
    // The last completed daily candle closes exactly at previous_close.
    let delta = movement(age) - movement(0);
    (base + delta * scale - base * age as i64 * scale / 1_800).max(base / 4)
}

// Demo history is anchored at 2026-10-14. It is deliberately independent of the
// machine date so screenshots and interaction tests remain reproducible.
fn historical_date(age: u64, period: Period) -> String {
    if period == Period::Month {
        let months = 2026 * 12 + 9 - age;
        return format!("{:04}/{:02}", months / 12, months % 12 + 1);
    }
    let mut remaining = if period == Period::Week {
        age * 7
    } else {
        age + 2 * ((age + 2) / 5)
    };
    let (mut year, mut month, mut day) = (2026, 10, 14);
    while remaining >= day as u64 {
        remaining -= day as u64;
        if month == 1 {
            year -= 1;
            month = 12;
        } else {
            month -= 1;
        }
        day = match month {
            4 | 6 | 9 | 11 => 30,
            2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
            2 => 28,
            _ => 31,
        };
    }
    day -= remaining as i32;
    format!("{month:02}/{day:02}")
}

/// Entire simulated session for indicators that must retain the opening
/// history even when the visible intraday window rolls forward.
pub fn session_bars(symbol: Symbol, tick: u64) -> Vec<Bar> {
    (0..=minute(tick))
        .map(|index| public_bar(minute_label(index), minute_candle(symbol, index)))
        .collect()
}

/// Returns bars in chronological order, always ending at the current quote.
pub fn bars(symbol: Symbol, period: Period, tick: u64) -> Vec<Bar> {
    if matches!(period, Period::Intraday | Period::Minute1) {
        let end = minute(tick);
        return (end + 1 - BAR_COUNT..=end)
            .map(|index| public_bar(minute_label(index), minute_candle(symbol, index)))
            .collect();
    }
    if period == Period::Minute5 {
        let end = minute(tick);
        return (0..=end)
            .step_by(5)
            .map(|start| {
                let mut combined = minute_candle(symbol, start);
                for index in start + 1..=(start + 4).min(end) {
                    let next = minute_candle(symbol, index);
                    combined.high = combined.high.max(next.high);
                    combined.low = combined.low.min(next.low);
                    combined.close = next.close;
                    combined.volume += next.volume;
                }
                public_bar(minute_label(start), combined)
            })
            .collect();
    }
    let snapshot = quote(symbol, tick);
    (0..BAR_COUNT)
        .rev()
        .map(|age| {
            if age == 0 && period == Period::Day {
                return Bar {
                    time: historical_date(age, period),
                    open: snapshot.open,
                    high: snapshot.high,
                    low: snapshot.low,
                    close: snapshot.price,
                    volume: snapshot.volume,
                };
            }
            let open = historical_close(symbol, age, period);
            let close = if age == 0 {
                minute_close(symbol, minute(tick))
            } else {
                historical_close(symbol, age - 1, period)
            };
            let wick = symbol.previous_close() / 150 + (noise(symbol.seed(), age) % 80) as i64;
            public_bar(
                historical_date(age, period),
                Candle {
                    open,
                    high: open.max(close) + wick,
                    low: open.min(close) - wick,
                    close,
                    volume: (12_000_000 + noise(symbol.seed() + 31, age) % 40_000_000)
                        * match period {
                            Period::Month => 21,
                            Period::Week => 5,
                            _ => 1,
                        },
                },
            )
        })
        .collect()
}

fn trade_price(symbol: Symbol, second: u64) -> i64 {
    let candle = minute_candle(symbol, second / 60);
    let progress = (second % 60) as i64;
    if progress == 59 {
        return candle.close;
    }
    let jitter = (noise(symbol.seed(), second) % 7) as i64 - 3;
    (candle.open + (candle.close - candle.open) * progress / 59 + jitter)
        .clamp(candle.low, candle.high)
}

/// Latest trades first. The latest execution matches the quote and chart.
pub fn trades(symbol: Symbol, tick: u64) -> Vec<Trade> {
    let last_second = minute(tick) * 60 + 59;
    (0..30)
        .map(|index| {
            let second = last_second - index * 3;
            let price = trade_price(symbol, second);
            let size = (1 + noise(symbol.seed() + 53, second) % 20) * 100;
            Trade {
                time: format!("{}:{:02}", minute_label(second / 60), second % 60),
                price: cents(price),
                size,
                buy: price >= trade_price(symbol, second - 1),
                count: 1 + noise(symbol.seed() + 59, second) % (size / 100).min(5),
            }
        })
        .collect()
}

/// Positive round-lot partitions, preserving a parent order/execution size.
fn split_lots(size: u64, count: u64, index: u64) -> u64 {
    let lots = size / 100;
    (lots / count + u64::from(index < lots % count)) * 100
}

/// Individual executions, newest first, matching the aggregate trade tape.
/// The two order-amount fields describe the originating buy/sell orders, so
/// each is at least the amount matched in this individual execution.
pub fn executions(symbol: Symbol, tick: u64) -> Vec<Execution> {
    trades(symbol, tick)
        .into_iter()
        .enumerate()
        .flat_map(|(row, trade)| {
            (0..trade.count).map(move |index| {
                let size = split_lots(trade.size, trade.count, index);
                let amount = trade.price * size as f64;
                let seed = minute(tick) * 1_000 + row as u64 * 10 + index;
                Execution {
                    time: trade.time.clone(),
                    price: trade.price,
                    size,
                    amount,
                    buy: trade.buy,
                    buy_order_amount: amount
                        + trade.price * (noise(symbol.seed() + 107, seed) % 20 * 100) as f64,
                    sell_order_amount: amount
                        + trade.price * (noise(symbol.seed() + 109, seed) % 20 * 100) as f64,
                }
            })
        })
        .collect()
}

fn second_label(second: u64) -> String {
    format!("{}:{:02}", minute_label(second / 60), second % 60)
}

/// Market-side pending and canceled orders, independent of DemoBroker.
/// Pending quantities at each price reproduce the displayed ten-level book;
/// cancellations are historical events and do not contribute to that depth.
pub fn market_orders(symbol: Symbol, tick: u64) -> Vec<MarketOrder> {
    let last_second = minute(tick) * 60 + 59;
    let (bids, asks) = order_book(symbol, tick);
    let mut orders = Vec::with_capacity(90);
    let mut next_id = minute(tick) * 1_000;
    for (buy, levels) in [(true, bids), (false, asks)] {
        for (level_index, level) in levels.into_iter().enumerate() {
            let count = (level.size / 100).min(3);
            for index in 0..count {
                next_id += 1;
                let size = split_lots(level.size, count, index);
                orders.push(MarketOrder {
                    id: next_id,
                    time: second_label(
                        last_second - level_index as u64 * 7 - index * 3 - u64::from(buy),
                    ),
                    price: level.price,
                    size,
                    amount: level.price * size as f64,
                    buy,
                    canceled_at: None,
                });
            }
        }
    }
    for index in 0..30 {
        next_id += 1;
        let canceled = last_second - index * 3;
        let submitted = canceled - 60 - noise(symbol.seed() + 113, canceled) % 300;
        let buy = index % 2 == 0;
        let price = cents(trade_price(symbol, submitted) + if buy { -1 } else { 1 });
        let size = (1 + noise(symbol.seed() + 127, submitted) % 90) * 100;
        orders.push(MarketOrder {
            id: next_id,
            time: second_label(submitted),
            price,
            size,
            amount: price * size as f64,
            buy,
            canceled_at: Some(second_label(canceled)),
        });
    }
    orders.sort_by(|a, b| {
        b.canceled_at
            .as_ref()
            .unwrap_or(&b.time)
            .cmp(a.canceled_at.as_ref().unwrap_or(&a.time))
            .then_with(|| b.id.cmp(&a.id))
    });
    orders
}

/// Ten bid and ask levels, each ordered best price first.
pub fn order_book(symbol: Symbol, tick: u64) -> (Vec<Level>, Vec<Level>) {
    let current_minute = minute(tick);
    let current = minute_close(symbol, current_minute);
    let side = |buy: bool| {
        (0..10)
            .map(|index| Level {
                price: cents(current + (index as i64 + 1) * if buy { -1 } else { 1 }),
                size: (1 + noise(
                    symbol.seed() + if buy { 67 } else { 89 },
                    current_minute * 10 + index,
                ) % 90)
                    * 100,
            })
            .collect()
    };
    (side(true), side(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_panels_share_one_deterministic_snapshot() {
        for symbol in Symbol::ALL {
            for tick in [0, 1, 76, 210, 211, 10_000, u64::MAX] {
                let snapshot = quote(symbol, tick);
                assert_eq!(snapshot, quote(symbol, tick));
                assert_eq!(trades(symbol, tick), trades(symbol, tick));
                assert_eq!(snapshot.price, trades(symbol, tick)[0].price);
                assert!((snapshot.price - snapshot.previous_close - snapshot.change).abs() < 1e-9);
                assert!(snapshot.low <= snapshot.price && snapshot.price <= snapshot.high);
                for period in Period::ALL {
                    let history = bars(symbol, period, tick);
                    assert_eq!(
                        history.len(),
                        if period == Period::Minute5 {
                            (minute(tick) / 5 + 1) as usize
                        } else {
                            BAR_COUNT as usize
                        }
                    );
                    assert_eq!(history.last().unwrap().close, snapshot.price);
                    assert_eq!(history, bars(symbol, period, tick));
                    for candle in history {
                        assert!(candle.low > 0.0);
                        assert!(candle.low <= candle.open.min(candle.close));
                        assert!(candle.high >= candle.open.max(candle.close));
                        assert!(candle.volume > 0);
                        if period == Period::Intraday {
                            assert!(candle.low >= snapshot.low);
                            assert!(candle.high <= snapshot.high);
                        }
                    }
                }
                let (bids, asks) = order_book(symbol, tick);
                assert_eq!(bids.len(), 10);
                assert_eq!(asks.len(), 10);
                assert!(bids[0].price < snapshot.price);
                assert!(asks[0].price > snapshot.price);
                assert!(bids.windows(2).all(|pair| pair[0].price > pair[1].price));
                assert!(asks.windows(2).all(|pair| pair[0].price < pair[1].price));
            }
        }
    }

    #[test]
    fn daily_bar_and_session_statistics_agree() {
        for symbol in Symbol::ALL {
            let snapshot = quote(symbol, 0);
            let history = bars(symbol, Period::Intraday, 0);
            let total_volume: u64 = history.iter().map(|bar| bar.volume).sum();
            assert_eq!(snapshot.volume, total_volume);
            assert!(snapshot.turnover >= snapshot.low * snapshot.volume as f64);
            assert!(snapshot.turnover <= snapshot.high * snapshot.volume as f64);
            let daily = bars(symbol, Period::Day, 0);
            let current = daily.last().unwrap();
            assert_eq!(current.open, snapshot.open);
            assert_eq!(current.high, snapshot.high);
            assert_eq!(current.low, snapshot.low);
            assert_eq!(current.volume, snapshot.volume);
            assert_eq!(daily[daily.len() - 2].close, snapshot.previous_close);
        }
    }

    #[test]
    fn advancing_demo_retains_completed_intraday_candles() {
        for symbol in Symbol::ALL {
            let before = bars(symbol, Period::Intraday, 0);
            let after = bars(symbol, Period::Intraday, 1);
            assert_eq!(before[1..], after[..after.len() - 1]);
            assert_ne!(quote(symbol, 0).price, quote(symbol, 7).price);
        }
    }

    #[test]
    fn historical_daily_labels_skip_weekends() {
        assert_eq!(historical_date(0, Period::Day), "10/14");
        assert_eq!(historical_date(2, Period::Day), "10/12");
        assert_eq!(historical_date(3, Period::Day), "10/09");
        assert_eq!(historical_date(10, Period::Day), "09/30");
    }

    #[test]
    fn expanded_statistics_balance_to_the_shared_quote() {
        for symbol in Symbol::ALL {
            for tick in [0, 76, 210, 211, u64::MAX] {
                let snapshot = quote(symbol, tick);
                assert_eq!(snapshot.buy_volume + snapshot.sell_volume, snapshot.volume);
                assert!(snapshot.float_shares > 0);
                assert!(snapshot.float_shares <= snapshot.total_shares);
                assert!(
                    (snapshot.market_cap - snapshot.price * snapshot.total_shares as f64).abs()
                        < 0.01
                );
                assert!(
                    (snapshot.float_market_cap - snapshot.price * snapshot.float_shares as f64)
                        .abs()
                        < 0.01
                );
                assert!(
                    (snapshot.turnover_rate * snapshot.float_shares as f64 / 100.0
                        - snapshot.volume as f64)
                        .abs()
                        < 1e-6
                );
                assert!(snapshot.volume_ratio.is_finite() && snapshot.volume_ratio > 0.0);
                assert!(
                    (snapshot.pe_ttm * symbol.earnings_per_share() - snapshot.price).abs() < 1e-9
                );
            }
        }
    }

    #[test]
    fn executions_reconcile_with_the_aggregated_tape() {
        for symbol in Symbol::ALL {
            for tick in [0, 210, 211, u64::MAX] {
                let details = executions(symbol, tick);
                assert_eq!(details, executions(symbol, tick));
                assert_eq!(details[0].price, quote(symbol, tick).price);
                for summary in trades(symbol, tick) {
                    let matching: Vec<_> = details
                        .iter()
                        .filter(|row| row.time == summary.time)
                        .collect();
                    assert_eq!(matching.len() as u64, summary.count);
                    assert_eq!(
                        matching.iter().map(|row| row.size).sum::<u64>(),
                        summary.size
                    );
                    let total = matching.iter().map(|row| row.amount).sum::<f64>();
                    assert!((total - summary.price * summary.size as f64).abs() < 1e-6);
                    for row in matching {
                        assert_eq!(row.price, summary.price);
                        assert_eq!(row.buy, summary.buy);
                        assert!(row.size > 0);
                        assert!(row.buy_order_amount >= row.amount);
                        assert!(row.sell_order_amount >= row.amount);
                    }
                }
            }
        }
    }

    #[test]
    fn pending_orders_reproduce_depth_and_cancellations_keep_original_fields() {
        for symbol in Symbol::ALL {
            for tick in [0, 76, 210, 211, u64::MAX] {
                let orders = market_orders(symbol, tick);
                assert_eq!(orders, market_orders(symbol, tick));
                let ids: std::collections::HashSet<_> = orders.iter().map(|row| row.id).collect();
                assert_eq!(ids.len(), orders.len());
                for row in &orders {
                    assert!(row.size > 0 && row.price > 0.0);
                    assert!((row.amount - row.price * row.size as f64).abs() < 1e-9);
                    if let Some(canceled) = &row.canceled_at {
                        assert!(canceled > &row.time);
                    }
                }
                let (bids, asks) = order_book(symbol, tick);
                for (buy, levels) in [(true, bids), (false, asks)] {
                    for level in levels {
                        let pending_size: u64 = orders
                            .iter()
                            .filter(|row| {
                                row.buy == buy
                                    && row.price == level.price
                                    && row.canceled_at.is_none()
                            })
                            .map(|row| row.size)
                            .sum();
                        assert_eq!(pending_size, level.size);
                    }
                }
                assert_eq!(
                    orders
                        .iter()
                        .filter(|row| row.canceled_at.is_some())
                        .count(),
                    30
                );
            }
        }
    }

    #[test]
    fn indicator_history_retains_session_open_when_visible_window_rolls() {
        for symbol in Symbol::ALL {
            let initial = session_bars(symbol, 0);
            for tick in [1, 76, 210] {
                let full = session_bars(symbol, tick);
                let visible = bars(symbol, Period::Intraday, tick);
                assert_eq!(&full[..initial.len()], initial.as_slice());
                assert_eq!(&full[full.len() - visible.len()..], visible.as_slice());
                assert_eq!(
                    full.iter().map(|bar| bar.volume).sum::<u64>(),
                    quote(symbol, tick).volume
                );
            }
        }
    }

    #[test]
    fn five_minute_candles_reconcile_the_entire_session() {
        for symbol in Symbol::ALL {
            for tick in [0, 1, 76, 210, 211, u64::MAX] {
                let snapshot = quote(symbol, tick);
                let history = bars(symbol, Period::Minute5, tick);
                assert_eq!(
                    history.iter().map(|row| row.volume).sum::<u64>(),
                    snapshot.volume
                );
                assert_eq!(history.first().unwrap().open, snapshot.open);
                assert_eq!(history.last().unwrap().close, snapshot.price);
                assert_eq!(
                    history
                        .iter()
                        .map(|row| row.high)
                        .fold(f64::NEG_INFINITY, f64::max),
                    snapshot.high
                );
                assert_eq!(
                    history
                        .iter()
                        .map(|row| row.low)
                        .fold(f64::INFINITY, f64::min),
                    snapshot.low
                );
                assert!(history.windows(2).all(|pair| pair[0].close == pair[1].open));
            }
        }
    }

    #[test]
    fn added_periods_keep_old_layout_values_and_months_cross_years() {
        for (serialized, expected) in [
            ("Intraday", Period::Intraday),
            ("Day", Period::Day),
            ("Week", Period::Week),
        ] {
            assert_eq!(
                serde_json::from_value::<Period>(serde_json::json!(serialized)).unwrap(),
                expected
            );
        }
        for period in Period::ALL {
            assert_eq!(
                serde_json::from_value::<Period>(serde_json::to_value(period).unwrap()).unwrap(),
                period
            );
        }
        assert_eq!(historical_date(0, Period::Month), "2026/10");
        assert_eq!(historical_date(10, Period::Month), "2025/12");
        assert_eq!(historical_date(179, Period::Month), "2011/11");
    }
}
