use super::*;

/// Compute indicators against the full history, then slice them with the bars.
/// Panning or zooming must not restart an MA or a session average at the edge.
pub(super) struct Indicators {
    pub(super) session: Vec<f64>,
    pub(super) price: [Vec<Option<f64>>; 5],
    pub(super) volume: [Vec<Option<f64>>; 2],
}

impl Indicators {
    pub(super) fn for_projection(data: &ChartData) -> Self {
        if data.indicator_prefix.is_empty() {
            return Self::new(&data.bars);
        }
        let complete: Vec<_> = data
            .indicator_prefix
            .iter()
            .chain(&data.bars)
            .cloned()
            .collect();
        Self::new(&complete).slice(data.indicator_prefix.len()..complete.len())
    }

    pub(super) fn new(bars: &[Bar]) -> Self {
        let prices: Vec<_> = bars.iter().map(|bar| bar.close).collect();
        let volumes: Vec<_> = bars.iter().map(|bar| bar.volume as f64).collect();
        Self {
            session: session_averages(bars),
            price: MA_PERIODS.map(|period| moving_average(&prices, period)),
            volume: [5, 10].map(|period| moving_average(&volumes, period)),
        }
    }

    pub(super) fn slice(&self, range: Range<usize>) -> Self {
        Self {
            session: self.session[range.clone()].to_vec(),
            price: std::array::from_fn(|i| self.price[i][range.clone()].to_vec()),
            volume: std::array::from_fn(|i| self.volume[i][range.clone()].to_vec()),
        }
    }
}

pub(super) fn bar_turnover(bar: &Bar) -> f64 {
    let mean_cents = ((bar.open + bar.high + bar.low + bar.close) * 100.).round() as u64 / 4;
    (mean_cents * bar.volume) as f64 / 100.
}

pub(super) fn session_averages(bars: &[Bar]) -> Vec<f64> {
    let mut value = 0.;
    let mut volume = 0.;
    bars.iter()
        .map(|bar| {
            let weight = bar.volume as f64;
            // The local feed estimates minute turnover from OHLC in integer
            // cents. Use that same convention so the chart and quote agree.
            value += bar_turnover(bar);
            volume += weight;
            if volume > 0. {
                value / volume
            } else {
                bar.close
            }
        })
        .collect()
}

pub(super) fn moving_average(values: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut sum = 0.;
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            sum += value;
            if index >= period {
                sum -= values[index - period];
            }
            (index + 1 >= period).then_some(sum / period as f64)
        })
        .collect()
}

pub(super) fn change_percent(price: f64, reference_price: f64) -> f64 {
    if reference_price > 0. && (price - reference_price).abs() > 0.000_000_1 {
        (price - reference_price) / reference_price * 100.
    } else {
        0.
    }
}

pub(super) fn price_extent(
    bars: &[Bar],
    indicators: &Indicators,
    intraday: bool,
    reference_price: f64,
) -> (f64, f64) {
    let mut minimum = if intraday {
        reference_price
    } else {
        f64::INFINITY
    };
    let mut maximum = if intraday {
        reference_price
    } else {
        f64::NEG_INFINITY
    };
    for bar in bars {
        minimum = minimum.min(if intraday { bar.close } else { bar.low });
        maximum = maximum.max(if intraday { bar.close } else { bar.high });
    }
    if intraday {
        for mean in &indicators.session {
            minimum = minimum.min(*mean);
            maximum = maximum.max(*mean);
        }
        // Equal distances above/below yesterday's close keep price and percent
        // axes aligned, even when the entire session trades on one side.
        let distance = (maximum - reference_price)
            .abs()
            .max((minimum - reference_price).abs())
            * 1.12;
        let distance = distance.max(reference_price.abs() * 0.002).max(0.01);
        return (reference_price - distance, reference_price + distance);
    }
    for mean in indicators
        .price
        .iter()
        .flat_map(|series| series.iter().flatten())
    {
        minimum = minimum.min(*mean);
        maximum = maximum.max(*mean);
    }
    let padding = ((maximum - minimum) * 0.08)
        .max(maximum.abs() * 0.0002)
        .max(0.01);
    (minimum - padding, maximum + padding)
}
