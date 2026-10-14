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
}

impl Period {
    pub const ALL: [Self; 3] = [Self::Intraday, Self::Day, Self::Week];

    pub fn label(self) -> &'static str {
        match self {
            Self::Intraday => "分时",
            Self::Day => "日 K",
            Self::Week => "周 K",
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
    let mut turnover_cents = 0_u64;
    for index in 0..=last_minute {
        let candle = minute_candle(symbol, index);
        high = high.max(candle.high);
        low = low.min(candle.low);
        volume += candle.volume;
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
    }
}

fn historical_close(symbol: Symbol, age: u64, period: Period) -> i64 {
    let base = symbol.previous_close();
    let scale = if period == Period::Week { 3 } else { 1 };
    let movement =
        |at| wave(symbol.seed(), at, 23, base / 15) + wave(symbol.seed() + 41, at, 5, base / 50);
    // The last completed daily candle closes exactly at previous_close.
    let delta = movement(age) - movement(0);
    (base + delta * scale - base * age as i64 * scale / 1_800).max(base / 4)
}

// Demo history is anchored at 2026-10-14. It is deliberately independent of the
// machine date so screenshots and interaction tests remain reproducible.
fn historical_date(age: u64, period: Period) -> String {
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

/// Returns bars in chronological order, always ending at the current quote.
pub fn bars(symbol: Symbol, period: Period, tick: u64) -> Vec<Bar> {
    if period == Period::Intraday {
        let end = minute(tick);
        return (end + 1 - BAR_COUNT..=end)
            .map(|index| public_bar(minute_label(index), minute_candle(symbol, index)))
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
                        * if period == Period::Week { 5 } else { 1 },
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
            Trade {
                time: format!("{}:{:02}", minute_label(second / 60), second % 60),
                price: cents(price),
                size: (1 + noise(symbol.seed() + 53, second) % 20) * 100,
                buy: price >= trade_price(symbol, second - 1),
            }
        })
        .collect()
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
                    assert_eq!(history.len(), BAR_COUNT as usize);
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
}
