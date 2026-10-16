//! Deterministic local market data shared by every panel.
//!
//! `tick` advances one wall-clock minute within a fixed demonstration day.
//! A shares begin at 10:00 and other markets at 12:29. Sessions stop at close;
//! lunch and unmatched auctions never invent trades. Prices use integer cents.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Symbol {
    Aapl,
    Msft,
    Nvda,
    Tsla,
    Tencent,
    Alibaba,
    Yinli,
    Yangguang,
    Catl,
    Moutai,
    Seres,
    Byd,
    Eastmoney,
    Smic,
    Midea,
    Luxshare,
    Cmb,
    Wuliangye,
}

impl Symbol {
    pub const LEGACY_DEFAULTS: [Self; 6] = [
        Self::Aapl,
        Self::Msft,
        Self::Nvda,
        Self::Tsla,
        Self::Tencent,
        Self::Alibaba,
    ];

    pub const ALL: [Self; 18] = [
        Self::Aapl,
        Self::Msft,
        Self::Nvda,
        Self::Tsla,
        Self::Tencent,
        Self::Alibaba,
        Self::Yinli,
        Self::Yangguang,
        Self::Catl,
        Self::Moutai,
        Self::Seres,
        Self::Byd,
        Self::Eastmoney,
        Self::Smic,
        Self::Midea,
        Self::Luxshare,
        Self::Cmb,
        Self::Wuliangye,
    ];

    /// A-share demonstration directory, in the same order as the design preview.
    /// Expanding this catalog does not append members to existing saved groups.
    pub const A_SHARES: [Self; 12] = [
        Self::Yinli,
        Self::Yangguang,
        Self::Catl,
        Self::Moutai,
        Self::Seres,
        Self::Byd,
        Self::Eastmoney,
        Self::Smic,
        Self::Midea,
        Self::Luxshare,
        Self::Cmb,
        Self::Wuliangye,
    ];

    pub fn is_a_share(self) -> bool {
        Self::A_SHARES.contains(&self)
    }

    /// Exact catalog code lookup. Numeric codes stay strings so leading zeroes
    /// retain their identity; display names and exchange prefixes are handled by
    /// the separate stock import parser.
    pub fn from_code(code: &str) -> Option<Self> {
        let code = code.trim();
        Self::ALL
            .into_iter()
            .find(|symbol| symbol.code().eq_ignore_ascii_case(code))
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::Aapl => "AAPL",
            Self::Msft => "MSFT",
            Self::Nvda => "NVDA",
            Self::Tsla => "TSLA",
            Self::Tencent => "00700",
            Self::Alibaba => "09988",
            Self::Yinli => "603598",
            Self::Yangguang => "001318",
            Self::Catl => "300750",
            Self::Moutai => "600519",
            Self::Seres => "601127",
            Self::Byd => "002594",
            Self::Eastmoney => "300059",
            Self::Smic => "688981",
            Self::Midea => "000333",
            Self::Luxshare => "002475",
            Self::Cmb => "600036",
            Self::Wuliangye => "000858",
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
            Self::Yinli => "引力传媒",
            Self::Yangguang => "阳光乳业",
            Self::Catl => "宁德时代",
            Self::Moutai => "贵州茅台",
            Self::Seres => "赛力斯",
            Self::Byd => "比亚迪",
            Self::Eastmoney => "东方财富",
            Self::Smic => "中芯国际",
            Self::Midea => "美的集团",
            Self::Luxshare => "立讯精密",
            Self::Cmb => "招商银行",
            Self::Wuliangye => "五粮液",
        }
    }

    pub fn sector(self) -> &'static str {
        match self {
            Self::Aapl | Self::Luxshare => "消费电子",
            Self::Msft => "软件服务",
            Self::Nvda | Self::Smic => "半导体",
            Self::Tsla | Self::Seres | Self::Byd => "汽车整车",
            Self::Tencent | Self::Alibaba => "互联网服务",
            Self::Yinli => "文化传媒",
            Self::Yangguang => "食品饮料",
            Self::Catl => "电池",
            Self::Moutai | Self::Wuliangye => "白酒",
            Self::Eastmoney => "证券",
            Self::Midea => "家用电器",
            Self::Cmb => "银行",
        }
    }

    pub fn market(self) -> &'static str {
        match self {
            Self::Tencent | Self::Alibaba => "HK",
            Self::Yinli | Self::Moutai | Self::Seres | Self::Smic | Self::Cmb => "SH",
            Self::Yangguang
            | Self::Catl
            | Self::Byd
            | Self::Eastmoney
            | Self::Midea
            | Self::Luxshare
            | Self::Wuliangye => "SZ",
            Self::Aapl | Self::Msft | Self::Nvda | Self::Tsla => "US",
        }
    }

    pub fn currency(self) -> &'static str {
        match self {
            Self::Tencent | Self::Alibaba => "HKD",
            Self::Aapl | Self::Msft | Self::Nvda | Self::Tsla => "USD",
            _ => "CNY",
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
            Self::Yinli => 157,
            Self::Yangguang => 181,
            Self::Catl => 211,
            Self::Moutai => 239,
            Self::Seres => 269,
            Self::Byd => 293,
            Self::Eastmoney => 317,
            Self::Smic => 347,
            Self::Midea => 373,
            Self::Luxshare => 401,
            Self::Cmb => 431,
            Self::Wuliangye => 457,
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
            Self::Yinli => 1_992,
            Self::Yangguang => 1_403,
            Self::Catl => 26_260,
            Self::Moutai => 147_841,
            Self::Seres => 11_859,
            Self::Byd => 10_748,
            Self::Eastmoney => 2_828,
            Self::Smic => 9_712,
            Self::Midea => 7_880,
            Self::Luxshare => 5_284,
            Self::Cmb => 4_305,
            Self::Wuliangye => 12_570,
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
            Self::Yinli => 150,
            Self::Yangguang => 92,
            Self::Catl => 620,
            Self::Moutai => -961,
            Self::Seres => 499,
            Self::Byd => -120,
            Self::Eastmoney => 103,
            Self::Smic => 124,
            Self::Midea => -65,
            Self::Luxshare => 112,
            Self::Cmb => -16,
            Self::Wuliangye => 60,
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
                Self::Yinli | Self::Yangguang | Self::Moutai | Self::Cmb | Self::Wuliangye => 100,
                Self::Catl | Self::Midea | Self::Luxshare => 95,
                Self::Seres | Self::Eastmoney => 90,
                Self::Byd => 75,
                Self::Smic => 80,
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
                Self::Yinli => 42.10,
                Self::Yangguang => 47.01,
                Self::Catl => 25.60,
                Self::Moutai => 23.40,
                Self::Seres => 48.20,
                Self::Byd => 24.80,
                Self::Eastmoney => 42.60,
                Self::Smic => 85.30,
                Self::Midea => 14.20,
                Self::Luxshare => 22.80,
                Self::Cmb => 6.80,
                Self::Wuliangye => 17.50,
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
            Self::Yinli => 267_862_800,
            Self::Yangguang => 283_200_000,
            Self::Catl => 4_405_000_000,
            Self::Moutai => 1_256_197_800,
            Self::Seres => 1_634_000_000,
            Self::Byd => 9_117_000_000,
            Self::Eastmoney => 15_838_000_000,
            Self::Smic => 7_986_000_000,
            Self::Midea => 7_027_000_000,
            Self::Luxshare => 7_237_000_000,
            Self::Cmb => 25_219_845_601,
            Self::Wuliangye => 3_881_608_005,
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

/// A frozen local demonstration of one daily candle's intraday session.
///
/// `day_index` addresses the chronological array returned by `bars(..., Day,
/// ...)`, rather than the visible/panned chart slice. Dates skip weekends, but
/// are not an exchange holiday calendar. Intraday clocks follow the symbol
/// market, including A-share auctions and the A-share/Hong Kong lunch breaks.
#[derive(Clone, Debug, PartialEq)]
pub struct HistoricalSession {
    pub day_index: usize,
    pub date: String,
    pub weekday: &'static str,
    pub daily: Bar,
    pub previous_close: f64,
    pub bars: Vec<Bar>,
    pub indicative: Vec<IndicativePoint>,
    pub wall_minute: u16,
    pub turnover: f64,
    pub turnover_rate: f64,
    pub previous_day: Option<usize>,
    pub next_day: Option<usize>,
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

/// Local exchange wall-clock minutes, independent of machine timezone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionPhase {
    BeforeOpen,
    OpeningAuctionCancelable,
    OpeningAuctionLocked,
    AwaitingOpen,
    Morning,
    LunchBreak,
    Afternoon,
    ClosingAuction,
    Closed,
}

impl SessionPhase {
    pub fn label(self) -> &'static str {
        match self {
            Self::BeforeOpen => "未开盘",
            Self::OpeningAuctionCancelable => "开盘竞价 · 可撤单",
            Self::OpeningAuctionLocked => "开盘竞价 · 不可撤单",
            Self::AwaitingOpen => "竞价已撮合 · 等待开盘",
            Self::Morning => "早盘交易",
            Self::LunchBreak => "午间休市",
            Self::Afternoon => "午盘交易",
            Self::ClosingAuction => "收盘竞价 · 不可撤单",
            Self::Closed => "已收盘",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionState {
    pub wall_minute: u16,
    pub phase: SessionPhase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionAxis {
    pub start: u16,
    pub end: u16,
    pub lunch: Option<(u16, u16)>,
    pub opening_auction: Option<(u16, u16)>,
    pub closing_auction: Option<(u16, u16)>,
}

pub fn session_axis(symbol: Symbol) -> SessionAxis {
    if symbol.is_a_share() {
        SessionAxis {
            start: 555,
            end: 900,
            lunch: Some((690, 780)),
            opening_auction: Some((555, 565)),
            closing_auction: Some((897, 900)),
        }
    } else {
        SessionAxis {
            start: 570,
            end: 960,
            lunch: (symbol.market() == "HK").then_some((720, 780)),
            opening_auction: None,
            closing_auction: None,
        }
    }
}

/// Phase at an explicit local exchange minute. 09:25 and 15:00 are the
/// A-share auction matching instants; indicative quotes before them are not trades.
pub fn session_at(symbol: Symbol, wall_minute: u16) -> SessionState {
    let axis = session_axis(symbol);
    let phase = if symbol.is_a_share() {
        match wall_minute {
            0..=554 => SessionPhase::BeforeOpen,
            555..=559 => SessionPhase::OpeningAuctionCancelable,
            560..=564 => SessionPhase::OpeningAuctionLocked,
            565..=569 => SessionPhase::AwaitingOpen,
            570..=689 => SessionPhase::Morning,
            690..=779 => SessionPhase::LunchBreak,
            780..=896 => SessionPhase::Afternoon,
            897..=899 => SessionPhase::ClosingAuction,
            _ => SessionPhase::Closed,
        }
    } else if wall_minute < axis.start {
        SessionPhase::BeforeOpen
    } else if wall_minute >= axis.end {
        SessionPhase::Closed
    } else if let Some((start, end)) = axis.lunch {
        if wall_minute < start {
            SessionPhase::Morning
        } else if wall_minute < end {
            SessionPhase::LunchBreak
        } else {
            SessionPhase::Afternoon
        }
    } else if wall_minute < 720 {
        SessionPhase::Morning
    } else {
        SessionPhase::Afternoon
    };
    SessionState { wall_minute, phase }
}

pub fn session_state(symbol: Symbol, tick: u64) -> SessionState {
    let start: u64 = if symbol.is_a_share() { 600 } else { 749 };
    let wall_minute = start
        .saturating_add(tick)
        .min(session_axis(symbol).end as u64) as u16;
    session_at(symbol, wall_minute)
}

pub fn minute_text(wall_minute: u16) -> String {
    format!("{:02}:{:02}", wall_minute / 60, wall_minute % 60)
}

pub fn bar_wall_minute(bar: &Bar) -> u16 {
    let (hour, minute) = bar
        .time
        .split_once(':')
        .expect("intraday bar has HH:mm time");
    hour.parse::<u16>().expect("numeric hour") * 60 + minute.parse::<u16>().expect("numeric minute")
}

fn session_minutes(symbol: Symbol) -> Vec<u16> {
    if symbol.is_a_share() {
        std::iter::once(565)
            .chain(570..=690)
            .chain(780..=897)
            .chain(std::iter::once(900))
            .collect()
    } else if symbol.market() == "HK" {
        (570..=720).chain(780..=960).collect()
    } else {
        (570..=960).collect()
    }
}

fn minute(symbol: Symbol, tick: u64) -> u64 {
    let wall_minute = session_state(symbol, tick).wall_minute;
    (session_minutes(symbol).partition_point(|minute| *minute <= wall_minute) - 1) as u64
}

fn auction_match(symbol: Symbol, index: u64) -> bool {
    // 09:25 match + 121 morning endpoints + 118 afternoon endpoints +
    // 15:00 match = 241 observations; only the first/last are call auctions.
    symbol.is_a_share() && matches!(index, 0 | 240)
}

#[derive(Clone, Debug, PartialEq)]
pub struct IndicativePoint {
    pub wall_minute: u16,
    pub price: f64,
}

fn indicative_at(
    symbol: Symbol,
    wall_minute: u16,
    opening: f64,
    closing: f64,
    seed: u64,
) -> Vec<IndicativePoint> {
    if !symbol.is_a_share() {
        return Vec::new();
    }
    (555..=564)
        .chain(897..=899)
        .filter(|at| *at <= wall_minute)
        .map(|at| {
            let target = if at < 570 { opening } else { closing };
            let until_match = if at < 570 { 565 - at } else { 900 - at };
            let offset = (noise(seed, at as u64) % 9) as i64 - 4;
            IndicativePoint {
                wall_minute: at,
                price: ((target * 100.).round() as i64 + offset * until_match as i64).max(1) as f64
                    / 100.,
            }
        })
        .collect()
}

/// Indicative auction prices are deliberately separate from executed bars,
/// daily OHLC, turnover and volume. Matching occurs once at 09:25 / 15:00.
pub fn auction_indicative(symbol: Symbol, tick: u64) -> Vec<IndicativePoint> {
    let minutes = session_minutes(symbol);
    indicative_at(
        symbol,
        session_state(symbol, tick).wall_minute,
        cents(minute_candle(symbol, 0).close),
        cents(minute_candle(symbol, minutes.len() as u64 - 1).close),
        symbol.seed(),
    )
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
    let open = if auction_match(symbol, index) {
        close
    } else if index == 0 {
        symbol.previous_close() + symbol.drift() / 3
    } else {
        minute_close(symbol, index - 1)
    };
    let wick = if auction_match(symbol, index) {
        0
    } else {
        2 + (noise(symbol.seed() + 17, index) % 16) as i64
    };
    Candle {
        open,
        high: open.max(close) + wick,
        low: open.min(close) - wick,
        close,
        volume: (180 + noise(symbol.seed() + 23, index) % 1_100) * 100,
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

fn minute_label(symbol: Symbol, index: u64) -> String {
    minute_text(session_minutes(symbol)[index as usize])
}

/// Snapshot statistics covering the simulated session up to `tick`.
pub fn quote(symbol: Symbol, tick: u64) -> Quote {
    let last_minute = minute(symbol, tick);
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
fn historical_date_parts(age: u64, period: Period) -> (i32, i32, i32) {
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
    (year, month, day)
}

fn historical_date(age: u64, period: Period) -> String {
    if period == Period::Month {
        let months = 2026 * 12 + 9 - age;
        return format!("{:04}/{:02}", months / 12, months % 12 + 1);
    }
    let (_, month, day) = historical_date_parts(age, period);
    format!("{month:02}/{day:02}")
}

/// Full date for one candle in the complete daily history. A visible chart
/// index must first be offset by its viewport's start. Out-of-range indices do
/// not wrap or silently select another day.
pub fn daily_date(day_index: usize) -> Option<String> {
    let age = (BAR_COUNT as usize).checked_sub(day_index.checked_add(1)?)? as u64;
    let (year, month, day) = historical_date_parts(age, Period::Day);
    Some(format!("{year:04}/{month:02}/{day:02}"))
}

fn completed_daily_candle(symbol: Symbol, age: u64) -> Candle {
    let open = historical_close(symbol, age, Period::Day);
    let close = historical_close(symbol, age - 1, Period::Day);
    let wick = symbol.previous_close() / 150 + (noise(symbol.seed(), age) % 80) as i64;
    Candle {
        open,
        high: open.max(close) + wick,
        low: open.min(close) - wick,
        close,
        volume: 12_000_000 + noise(symbol.seed() + 31, age) % 40_000_000,
    }
}

/// Expand an existing completed daily candle without inventing conflicting
/// high/low/close or total volume. Price paths visit both daily extremes, and
/// integer cumulative allocation preserves every share of the daily volume.
fn completed_session_bars(symbol: Symbol, age: u64, daily: Candle) -> Vec<Bar> {
    let last_minute = session_minutes(symbol).len() as u64 - 1;
    let seed = symbol.seed().wrapping_add(age * 337);
    let first_extreme = last_minute / 5 + noise(seed, 1) % (last_minute / 10);
    let second_extreme = last_minute * 3 / 5 + noise(seed, 2) % (last_minute / 8);
    let (first_price, second_price) = if daily.close >= daily.open {
        (daily.low, daily.high)
    } else {
        (daily.high, daily.low)
    };
    let anchors = [
        (0, daily.open),
        (first_extreme, first_price),
        (second_extreme, second_price),
        (last_minute, daily.close),
    ];
    let close_at = |index: u64| {
        let &(left_index, left_price) = anchors
            .iter()
            .rev()
            .find(|(position, _)| *position <= index)
            .unwrap();
        if index == left_index {
            return left_price;
        }
        let &(right_index, right_price) = anchors
            .iter()
            .find(|(position, _)| *position > index)
            .unwrap();
        let span = (right_index - left_index) as i64;
        let offset = (index - left_index) as i64;
        let trend = left_price + (right_price - left_price) * offset / span;
        let envelope = offset.min(span - offset);
        let movement =
            wave(seed + 7, index, 7, (daily.high - daily.low) / 20) * envelope * 2 / span;
        (trend + movement).clamp(daily.low, daily.high)
    };
    let weights: Vec<_> = (0..=last_minute)
        .map(|index| {
            50 + noise(seed + 11, index) % 100 + if !(30..=360).contains(&index) { 160 } else { 0 }
        })
        .collect();
    let weight_sum: u64 = weights.iter().sum();
    let mut weight_so_far = 0;
    let mut volume_so_far = 0;
    (0..=last_minute)
        .map(|index| {
            let open = if auction_match(symbol, index) {
                close_at(index)
            } else if index == 0 {
                daily.open
            } else {
                close_at(index - 1)
            };
            let close = close_at(index);
            let wick = if auction_match(symbol, index) {
                0
            } else {
                (noise(seed + 17, index) % 15) as i64
            };
            weight_so_far += weights[index as usize];
            let next_volume = daily.volume * weight_so_far / weight_sum;
            let volume = next_volume - volume_so_far;
            volume_so_far = next_volume;
            public_bar(
                minute_label(symbol, index),
                Candle {
                    open,
                    high: (open.max(close) + wick).min(daily.high),
                    low: (open.min(close) - wick).max(daily.low),
                    close,
                    volume,
                },
            )
        })
        .collect()
}

/// Inspect a daily candle using one fixed demo tick. The latest day is a
/// partial session matching the current quote at that tick; completed days
/// contain the full synthetic session and are independent of future ticks.
/// Previous/next indices stay within the available daily history.
pub fn historical_session(
    symbol: Symbol,
    day_index: usize,
    tick: u64,
) -> Option<HistoricalSession> {
    let date = daily_date(day_index)?;
    let age = BAR_COUNT - 1 - day_index as u64;
    let (daily, session, previous_close) = if age == 0 {
        let snapshot = quote(symbol, tick);
        (
            Bar {
                time: historical_date(0, Period::Day),
                open: snapshot.open,
                high: snapshot.high,
                low: snapshot.low,
                close: snapshot.price,
                volume: snapshot.volume,
            },
            session_bars(symbol, tick),
            snapshot.previous_close,
        )
    } else {
        let candle = completed_daily_candle(symbol, age);
        (
            public_bar(historical_date(age, Period::Day), candle),
            completed_session_bars(symbol, age, candle),
            cents(historical_close(symbol, age, Period::Day)),
        )
    };
    // The same integer-cent OHLC convention used by quote() and the chart's
    // cumulative session average. No turnover is taken from another day.
    let turnover_cents: u64 = session
        .iter()
        .map(|bar| {
            let mean_cents =
                ((bar.open + bar.high + bar.low + bar.close) * 100.).round() as u64 / 4;
            mean_cents * bar.volume
        })
        .sum();
    let weekday =
        ["星期一", "星期二", "星期三", "星期四", "星期五"][(2 + 5 - age % 5) as usize % 5];
    let indicative = if age == 0 {
        auction_indicative(symbol, tick)
    } else {
        indicative_at(
            symbol,
            session_axis(symbol).end,
            daily.open,
            daily.close,
            symbol.seed().wrapping_add(age * 337),
        )
    };
    Some(HistoricalSession {
        day_index,
        date,
        weekday,
        turnover_rate: daily.volume as f64 / symbol.float_shares() as f64 * 100.,
        daily,
        previous_close,
        indicative,
        wall_minute: if age == 0 {
            session_state(symbol, tick).wall_minute
        } else {
            session_axis(symbol).end
        },
        bars: session,
        turnover: turnover_cents as f64 / 100.,
        previous_day: day_index.checked_sub(1),
        next_day: (day_index + 1 < BAR_COUNT as usize).then_some(day_index + 1),
    })
}

/// Executed session bars only. Trading continues across the two A-share/HK
/// sessions, but never creates observations in lunch or unmatched auctions.
pub fn session_bars_at(symbol: Symbol, wall_minute: u16) -> Vec<Bar> {
    session_minutes(symbol)
        .into_iter()
        .take_while(|minute| *minute <= wall_minute)
        .enumerate()
        .map(|(index, minute)| public_bar(minute_text(minute), minute_candle(symbol, index as u64)))
        .collect()
}

pub fn session_bars(symbol: Symbol, tick: u64) -> Vec<Bar> {
    session_bars_at(symbol, session_state(symbol, tick).wall_minute)
}

/// Returns bars in chronological order. Intraday retains today's opening
/// history; it never scrolls old minutes away or loops back after close.
pub fn bars(symbol: Symbol, period: Period, tick: u64) -> Vec<Bar> {
    if period == Period::Intraday {
        return session_bars(symbol, tick);
    }
    if period == Period::Minute1 {
        let mut session = session_bars(symbol, tick);
        let start = session.len().saturating_sub(BAR_COUNT as usize);
        return session.drain(start..).collect();
    }
    if period == Period::Minute5 {
        let mut result: Vec<Bar> = Vec::new();
        let mut previous_bucket = None;
        for bar in session_bars(symbol, tick) {
            let at = bar_wall_minute(&bar);
            // Wall-clock bucket boundaries ensure morning, afternoon and
            // individual auction matches cannot merge through a market break.
            let bucket = at / 5;
            if previous_bucket == Some(bucket) {
                let combined = result.last_mut().unwrap();
                combined.high = combined.high.max(bar.high);
                combined.low = combined.low.min(bar.low);
                combined.close = bar.close;
                combined.volume += bar.volume;
            } else {
                previous_bucket = Some(bucket);
                result.push(bar);
            }
        }
        return result;
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
            if period == Period::Day {
                return public_bar(
                    historical_date(age, period),
                    completed_daily_candle(symbol, age),
                );
            }
            let open = historical_close(symbol, age, period);
            let close = if age == 0 {
                minute_close(symbol, minute(symbol, tick))
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

/// Logical price-progress positions with actual exchange timestamps. The
/// first print after a break and call-auction matches occur exactly on the
/// boundary; all other positions stay inside a continuous trading minute.
fn recent_execution_seconds(
    symbol: Symbol,
    last_index: u64,
    count: usize,
    step: usize,
    include_auctions: bool,
) -> Vec<u64> {
    let minutes = session_minutes(symbol);
    let mut result = Vec::with_capacity(count);
    for index in (0..=last_index).rev() {
        let is_auction = auction_match(symbol, index);
        if is_auction && !include_auctions {
            continue;
        }
        let isolated =
            is_auction || index == 0 || minutes[index as usize] != minutes[index as usize - 1] + 1;
        for offset in (0..if isolated { 1 } else { 60 }).step_by(step) {
            result.push(index * 60 + 59 - offset as u64);
            if result.len() == count {
                return result;
            }
        }
    }
    result
}

/// Latest trades first. The latest execution matches the quote and chart;
/// A-share lunch/auction waiting periods preserve the last executed trade.
pub fn trades(symbol: Symbol, tick: u64) -> Vec<Trade> {
    recent_execution_seconds(symbol, minute(symbol, tick), 30, 3, true)
        .into_iter()
        .map(|second| {
            let price = trade_price(symbol, second);
            let size = if auction_match(symbol, second / 60) {
                minute_candle(symbol, second / 60).volume
            } else {
                (1 + noise(symbol.seed() + 53, second) % 20) * 100
            };
            Trade {
                time: second_label(symbol, second),
                price: cents(price),
                size,
                buy: price >= trade_price(symbol, second.saturating_sub(1)),
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
                let seed = minute(symbol, tick) * 1_000 + row as u64 * 10 + index;
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

fn second_label(symbol: Symbol, second: u64) -> String {
    let index = second / 60;
    let wall_second = session_minutes(symbol)[index as usize] as u64 * 60 - (59 - second % 60);
    format!(
        "{}:{:02}",
        minute_text((wall_second / 60) as u16),
        wall_second % 60
    )
}

/// Market-side pending and canceled orders, independent of DemoBroker.
/// Pending quantities at each price reproduce the displayed ten-level book;
/// cancellations are historical events and do not contribute to that depth.
pub fn market_orders(symbol: Symbol, tick: u64) -> Vec<MarketOrder> {
    let recent = recent_execution_seconds(symbol, minute(symbol, tick), 500, 1, false);
    let (bids, asks) = order_book(symbol, tick);
    let mut orders = Vec::with_capacity(90);
    let mut next_id = minute(symbol, tick) * 1_000;
    for (buy, levels) in [(true, bids), (false, asks)] {
        for (level_index, level) in levels.into_iter().enumerate() {
            let count = (level.size / 100).min(3);
            for index in 0..count {
                next_id += 1;
                let size = split_lots(level.size, count, index);
                orders.push(MarketOrder {
                    id: next_id,
                    time: second_label(
                        symbol,
                        recent[level_index * 7 + index as usize * 3 + usize::from(buy)],
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
        let canceled = recent[index * 3 + 1];
        let submitted =
            recent[index * 3 + 60 + (noise(symbol.seed() + 113, canceled) % 300) as usize];
        let buy = index % 2 == 0;
        let price = cents(trade_price(symbol, submitted) + if buy { -1 } else { 1 });
        let size = (1 + noise(symbol.seed() + 127, submitted) % 90) * 100;
        orders.push(MarketOrder {
            id: next_id,
            time: second_label(symbol, submitted),
            price,
            size,
            amount: price * size as f64,
            buy,
            canceled_at: Some(second_label(symbol, canceled)),
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
    let current_minute = minute(symbol, tick);
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
    fn catalog_codes_round_trip_without_losing_leading_zeroes() {
        let codes: std::collections::HashSet<_> =
            Symbol::ALL.iter().map(|symbol| symbol.code()).collect();
        assert_eq!(codes.len(), Symbol::ALL.len());
        for symbol in Symbol::ALL {
            assert_eq!(Symbol::from_code(symbol.code()), Some(symbol));
            assert!(!symbol.name().is_empty());
            assert!(!symbol.sector().is_empty());
        }
        assert_eq!(Symbol::from_code("  aapl \n"), Some(Symbol::Aapl));
        assert_eq!(Symbol::from_code("00700"), Some(Symbol::Tencent));
        assert_eq!(Symbol::from_code("001318"), Some(Symbol::Yangguang));
        assert_eq!(Symbol::from_code("000333"), Some(Symbol::Midea));
        assert_eq!(Symbol::from_code("000858"), Some(Symbol::Wuliangye));
        for unknown in [
            "",
            "700",
            "1318",
            "333",
            "858",
            "SH600519",
            "贵州茅台",
            "999999",
        ] {
            assert_eq!(Symbol::from_code(unknown), None, "{unknown}");
        }
    }

    #[test]
    fn expanded_catalog_keeps_serialized_legacy_symbols_and_group_defaults() {
        let legacy = [
            ("Aapl", Symbol::Aapl),
            ("Msft", Symbol::Msft),
            ("Nvda", Symbol::Nvda),
            ("Tsla", Symbol::Tsla),
            ("Tencent", Symbol::Tencent),
            ("Alibaba", Symbol::Alibaba),
            ("Yinli", Symbol::Yinli),
            ("Yangguang", Symbol::Yangguang),
        ];
        for (index, (serialized, symbol)) in legacy.iter().enumerate() {
            assert_eq!(Symbol::ALL[index], *symbol);
            assert_eq!(
                serde_json::from_value::<Symbol>(serde_json::json!(serialized)).unwrap(),
                *symbol
            );
            assert_eq!(serde_json::to_value(symbol).unwrap(), *serialized);
        }
        assert_eq!(&Symbol::ALL[..6], &Symbol::LEGACY_DEFAULTS);
        for symbol in Symbol::ALL {
            assert_eq!(
                serde_json::from_value::<Symbol>(serde_json::to_value(symbol).unwrap()).unwrap(),
                symbol
            );
        }
    }

    #[test]
    fn a_share_directory_uses_cny_and_local_exchange_schedules() {
        let directory: std::collections::HashSet<_> = Symbol::A_SHARES.into_iter().collect();
        assert_eq!(directory.len(), 12);
        for symbol in Symbol::A_SHARES {
            assert!(Symbol::ALL.contains(&symbol));
            assert!(symbol.is_a_share());
            assert_eq!(symbol.currency(), "CNY");
            assert!(matches!(symbol.market(), "SH" | "SZ"));
            assert_eq!(symbol.code().len(), 6);
            assert!(symbol.code().bytes().all(|byte| byte.is_ascii_digit()));
            assert_eq!(session_axis(symbol).lunch, Some((690, 780)));
            assert_eq!(session_state(symbol, 0).wall_minute, 600);
            assert_eq!(session_state(symbol, u64::MAX).wall_minute, 900);
        }
        assert!(
            Symbol::LEGACY_DEFAULTS
                .into_iter()
                .all(|symbol| !symbol.is_a_share())
        );
    }

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
                    match period {
                        Period::Intraday => {
                            assert_eq!(history.len(), minute(symbol, tick) as usize + 1)
                        }
                        Period::Minute1 => assert_eq!(
                            history.len(),
                            (minute(symbol, tick) as usize + 1).min(BAR_COUNT as usize)
                        ),
                        Period::Minute5 => {
                            assert!(history.len() <= session_bars(symbol, tick).len())
                        }
                        _ => assert_eq!(history.len(), BAR_COUNT as usize),
                    }
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
            assert_eq!(before, after[..before.len()]);
            assert!(after.len() >= before.len());
            assert_ne!(quote(symbol, 0).price, quote(symbol, 40).price);
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
    fn historical_sessions_reproduce_each_selected_daily_candle() {
        for symbol in Symbol::ALL {
            let daily = bars(symbol, Period::Day, 76);
            for (day_index, expected) in daily.iter().enumerate() {
                let session = historical_session(symbol, day_index, 76).unwrap();
                assert_eq!(&session.daily, expected);
                assert_eq!(session.bars.first().unwrap().open, expected.open);
                assert_eq!(session.bars.last().unwrap().close, expected.close);
                assert_eq!(
                    session
                        .bars
                        .iter()
                        .map(|bar| bar.high)
                        .fold(f64::NEG_INFINITY, f64::max),
                    expected.high
                );
                assert_eq!(
                    session
                        .bars
                        .iter()
                        .map(|bar| bar.low)
                        .fold(f64::INFINITY, f64::min),
                    expected.low
                );
                assert_eq!(
                    session.bars.iter().map(|bar| bar.volume).sum::<u64>(),
                    expected.volume
                );
                assert!(
                    session
                        .bars
                        .windows(2)
                        .all(|pair| pair[0].close == pair[1].open
                            || (symbol.is_a_share() && pair[1].time == "15:00"))
                );
                assert!(session.bars.iter().all(|bar| {
                    bar.volume > 0
                        && bar.low > 0.
                        && bar.low <= bar.open.min(bar.close)
                        && bar.high >= bar.open.max(bar.close)
                }));
                let turnover = session
                    .bars
                    .iter()
                    .map(|bar| {
                        let cents =
                            ((bar.open + bar.high + bar.low + bar.close) * 100.).round() as u64 / 4;
                        cents * bar.volume
                    })
                    .sum::<u64>() as f64
                    / 100.;
                assert_eq!(session.turnover, turnover);
                assert!(session.turnover >= expected.low * expected.volume as f64);
                assert!(session.turnover <= expected.high * expected.volume as f64);
                if day_index > 0 {
                    assert_eq!(session.previous_close, daily[day_index - 1].close);
                }
                if session.next_day.is_some() {
                    assert_eq!(session.bars.len(), session_minutes(symbol).len());
                    assert_eq!(
                        session.bars.first().unwrap().time,
                        if symbol.is_a_share() {
                            "09:25"
                        } else {
                            "09:30"
                        }
                    );
                    assert_eq!(
                        session.bars.last().unwrap().time,
                        if symbol.is_a_share() {
                            "15:00"
                        } else {
                            "16:00"
                        }
                    );
                    assert_eq!(
                        session,
                        historical_session(symbol, day_index, u64::MAX).unwrap()
                    );
                }
            }
        }
    }

    #[test]
    fn latest_historical_session_freezes_the_requested_demo_tick() {
        for symbol in Symbol::ALL {
            for tick in [0, 1, 76, 210, 211, u64::MAX] {
                let snapshot = quote(symbol, tick);
                let session = historical_session(symbol, BAR_COUNT as usize - 1, tick).unwrap();
                assert_eq!(session.bars, session_bars(symbol, tick));
                assert_eq!(session.daily.close, snapshot.price);
                assert_eq!(session.previous_close, snapshot.previous_close);
                assert_eq!(session.turnover, snapshot.turnover);
                assert_eq!(session.turnover_rate, snapshot.turnover_rate);
                assert_eq!(
                    session,
                    historical_session(symbol, BAR_COUNT as usize - 1, tick).unwrap()
                );
            }
        }
    }

    #[test]
    fn historical_session_navigation_uses_bounded_full_daily_dates() {
        let session = |day| historical_session(Symbol::Aapl, day, 0).unwrap();
        let latest = session(179);
        assert_eq!(latest.date, "2026/10/14");
        assert_eq!(latest.weekday, "星期三");
        assert_eq!(latest.previous_day, Some(178));
        assert_eq!(latest.next_day, None);
        assert_eq!(session(177).date, "2026/10/12");
        let friday = session(session(177).previous_day.unwrap());
        assert_eq!(friday.date, "2026/10/09");
        assert_eq!(friday.weekday, "星期五");
        assert_eq!(friday.next_day, Some(177));
        let earliest = session(0);
        assert_eq!(earliest.date, "2026/02/05");
        assert_eq!(earliest.weekday, "星期四");
        assert_eq!(earliest.previous_day, None);
        assert_eq!(earliest.next_day, Some(1));
        for index in [180, usize::MAX] {
            assert!(daily_date(index).is_none());
            assert!(historical_session(Symbol::Aapl, index, 0).is_none());
        }
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
    fn intraday_and_indicators_keep_all_executed_minutes_from_open() {
        for symbol in Symbol::ALL {
            let initial = session_bars(symbol, 0);
            for tick in [1, 76, 210] {
                let full = session_bars(symbol, tick);
                let visible = bars(symbol, Period::Intraday, tick);
                assert_eq!(&full[..initial.len()], initial.as_slice());
                assert_eq!(full, visible);
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
                assert!(history.windows(2).all(|pair| pair[0].close == pair[1].open
                    || (symbol.is_a_share() && pair[1].time == "15:00")));
            }
        }
    }

    #[test]
    fn a_share_session_boundaries_separate_auction_waiting_and_lunch() {
        for symbol in Symbol::A_SHARES {
            for (minute, phase) in [
                (554, SessionPhase::BeforeOpen),
                (555, SessionPhase::OpeningAuctionCancelable),
                (559, SessionPhase::OpeningAuctionCancelable),
                (560, SessionPhase::OpeningAuctionLocked),
                (564, SessionPhase::OpeningAuctionLocked),
                (565, SessionPhase::AwaitingOpen),
                (569, SessionPhase::AwaitingOpen),
                (570, SessionPhase::Morning),
                (689, SessionPhase::Morning),
                (690, SessionPhase::LunchBreak),
                (779, SessionPhase::LunchBreak),
                (780, SessionPhase::Afternoon),
                (896, SessionPhase::Afternoon),
                (897, SessionPhase::ClosingAuction),
                (899, SessionPhase::ClosingAuction),
                (900, SessionPhase::Closed),
            ] {
                assert_eq!(session_at(symbol, minute).phase, phase);
            }
            assert!(session_bars_at(symbol, 564).is_empty());
            let matched_open = session_bars_at(symbol, 565);
            assert_eq!(matched_open.len(), 1);
            assert_eq!(matched_open[0].time, "09:25");
            assert_eq!(matched_open[0].open, matched_open[0].close);
            assert_eq!(matched_open[0].low, matched_open[0].high);
            assert_eq!(matched_open, session_bars_at(symbol, 569));
            assert_eq!(session_bars_at(symbol, 570).len(), 2);
            assert_eq!(session_bars_at(symbol, 690), session_bars_at(symbol, 779));
            assert_eq!(session_bars_at(symbol, 897), session_bars_at(symbol, 899));
            let before_close = session_bars_at(symbol, 899);
            let closed = session_bars_at(symbol, 900);
            assert_eq!(closed.len(), before_close.len() + 1);
            assert_eq!(&closed[..before_close.len()], before_close.as_slice());
            let closing_match = closed.last().unwrap();
            assert_eq!(closing_match.time, "15:00");
            assert_eq!(closing_match.open, closing_match.close);
            assert_eq!(closing_match.high, closing_match.low);
            assert_eq!(closed.iter().filter(|bar| bar.time == "09:25").count(), 1);
            assert_eq!(closed.iter().filter(|bar| bar.time == "15:00").count(), 1);
            assert_eq!(closed.len(), 241);
            assert_eq!(quote(symbol, 90), quote(symbol, 179));
            assert_eq!(quote(symbol, 297), quote(symbol, 299));
            assert!(quote(symbol, 297).volume > quote(symbol, 296).volume);
            let last_continuous_trades = trades(symbol, 297);
            assert_eq!(last_continuous_trades[0].time, "14:57:00");
            assert_eq!(last_continuous_trades[1].time, "14:56:57");
            assert_eq!(trades(symbol, 300)[0].time, "15:00:00");
            assert_eq!(trades(symbol, 300)[1].time, "14:57:00");
        }
    }

    #[test]
    fn auctions_never_contribute_indicative_prices_to_executed_statistics() {
        for symbol in Symbol::A_SHARES {
            let indicative = indicative_at(symbol, 564, 20., 21., symbol.seed());
            assert_eq!(indicative.len(), 10);
            assert!(session_bars_at(symbol, 564).is_empty());
            assert_eq!(auction_indicative(symbol, 296).len(), 10);
            assert_eq!(auction_indicative(symbol, 297).len(), 11);
            assert_eq!(auction_indicative(symbol, 299).len(), 13);
            assert_eq!(quote(symbol, 297), quote(symbol, 299));
            let last = session_bars(symbol, 300).pop().unwrap();
            assert_eq!(
                quote(symbol, 300).volume - quote(symbol, 299).volume,
                last.volume
            );
            assert!(
                (quote(symbol, 300).turnover
                    - quote(symbol, 299).turnover
                    - last.close * last.volume as f64)
                    .abs()
                    < 0.00001
            );
            let history = historical_session(symbol, 100, 0).unwrap();
            assert_eq!(history.indicative.len(), 13);
            assert_eq!(history.wall_minute, 900);
        }
    }

    #[test]
    fn all_markets_stop_at_close_without_wrapping_and_keep_distinct_schedules() {
        for symbol in Symbol::ALL {
            let initial = bars(symbol, Period::Intraday, 0);
            let closed = bars(symbol, Period::Intraday, 10_000);
            assert_eq!(&closed[..initial.len()], initial.as_slice());
            assert_eq!(closed, bars(symbol, Period::Intraday, u64::MAX));
            assert_eq!(quote(symbol, 10_000), quote(symbol, u64::MAX));
            assert_eq!(trades(symbol, 10_000), trades(symbol, u64::MAX));
            assert_eq!(session_state(symbol, u64::MAX).phase, SessionPhase::Closed);
        }
        let hk = session_bars(Symbol::Tencent, u64::MAX);
        assert_eq!(hk.len(), 332);
        assert!(
            hk.iter()
                .all(|bar| !(721..780).contains(&bar_wall_minute(bar)))
        );
        assert_eq!(
            session_at(Symbol::Tencent, 720).phase,
            SessionPhase::LunchBreak
        );
        assert_eq!(
            session_at(Symbol::Tencent, 780).phase,
            SessionPhase::Afternoon
        );
        let us = session_bars(Symbol::Aapl, u64::MAX);
        assert_eq!(us.len(), 391);
        assert!(us.iter().any(|bar| bar.time == "12:30"));
    }

    #[test]
    fn trade_and_order_timestamps_never_invent_executions_during_a_market_break() {
        for symbol in Symbol::A_SHARES.into_iter().chain([Symbol::Tencent]) {
            let axis = session_axis(symbol);
            let (lunch_start, lunch_end) = axis.lunch.unwrap();
            for tick in [0, 31, 90, 120, 179, 180, 181, 296, 297, 299, 300, u64::MAX] {
                let valid_time = |time: &str| {
                    let parts: Vec<u16> =
                        time.split(':').map(|part| part.parse().unwrap()).collect();
                    let second = parts[0] as u32 * 3600 + parts[1] as u32 * 60 + parts[2] as u32;
                    assert!(second <= session_state(symbol, tick).wall_minute as u32 * 60);
                    assert!(
                        second <= lunch_start as u32 * 60 || second >= lunch_end as u32 * 60,
                        "{symbol:?} {tick} {time}"
                    );
                    if symbol.is_a_share() {
                        assert!(
                            second == 565 * 60
                                || (570 * 60..=897 * 60).contains(&second)
                                || second == 900 * 60,
                            "{time}"
                        );
                    }
                };
                for trade in trades(symbol, tick) {
                    valid_time(&trade.time);
                }
                for execution in executions(symbol, tick) {
                    valid_time(&execution.time);
                }
                for order in market_orders(symbol, tick) {
                    valid_time(&order.time);
                    if let Some(canceled) = order.canceled_at {
                        valid_time(&canceled);
                    }
                }
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
