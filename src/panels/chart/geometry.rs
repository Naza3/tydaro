//! Shared clock coordinates for painting and hit testing.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Geometry {
    pub(super) left: f32,
    pub(super) top: f32,
    pub(super) width: f32,
    pub(super) price_height: f32,
    pub(super) volume_top: f32,
    pub(super) bottom: f32,
}

impl Geometry {
    pub(super) fn new(bounds: Bounds<Pixels>) -> Self {
        let height = bounds.size.height.as_f32();
        let top = bounds.origin.y.as_f32() + 24.;
        let price_height = ((height - 62.) * 0.76).max(8.);
        Self {
            left: bounds.origin.x.as_f32() + 55.,
            top,
            width: (bounds.size.width.as_f32() - 113.).max(1.),
            price_height,
            volume_top: top + price_height + 22.,
            bottom: bounds.origin.y.as_f32() + height - 20.,
        }
    }
    pub(super) fn right(self) -> f32 {
        self.left + self.width
    }
    pub(super) fn price_bottom(self) -> f32 {
        self.top + self.price_height
    }
    pub(super) fn x(self, index: usize, count: usize) -> f32 {
        self.left + (index as f32 + 0.5) * self.width / count.max(1) as f32
    }
    pub(super) fn index(self, pointer: Point<Pixels>, count: usize) -> Option<usize> {
        if count == 0
            || pointer.x.as_f32() < self.left
            || pointer.x.as_f32() > self.right()
            || pointer.y.as_f32() < self.top
            || pointer.y.as_f32() > self.bottom
        {
            return None;
        }
        Some(
            (((pointer.x.as_f32() - self.left) / self.width) * count as f32)
                .floor()
                .max(0.) as usize,
        )
        .map(|i| i.min(count - 1))
    }
}

/// Fixed market-clock coordinates. Empty future minutes and breaks keep their
/// positions even though they have no trade bar yet.
pub(super) struct SessionTimeline {
    pub(super) axis: SessionAxis,
}

impl SessionTimeline {
    pub(super) fn new(axis: SessionAxis) -> Self {
        Self { axis }
    }

    pub(super) fn cash_start(&self) -> u16 {
        self.axis
            .opening_auction
            .map_or(self.axis.start, |(_, end)| end + 5)
    }

    pub(super) fn auction_width(&self, geometry: Geometry) -> f32 {
        if self.axis.opening_auction.is_some() {
            (geometry.width * 0.14)
                .clamp(32., 100.)
                .min(geometry.width * 0.28)
        } else {
            0.
        }
    }

    pub(super) fn opening_gap(&self) -> f32 {
        if self.axis.opening_auction.is_some() {
            8.
        } else {
            0.
        }
    }
    pub(super) fn lunch_gap(&self) -> f32 {
        if self.axis.lunch.is_some() { 8. } else { 0. }
    }

    pub(super) fn cash_minutes(&self) -> u16 {
        self.axis.end - self.cash_start() - self.axis.lunch.map_or(0, |(start, end)| end - start)
    }

    pub(super) fn cash_width(&self, geometry: Geometry) -> f32 {
        (geometry.width - self.auction_width(geometry) - self.opening_gap() - self.lunch_gap())
            .max(1.)
    }

    pub(super) fn x(&self, minute: u16, geometry: Geometry) -> f32 {
        let auction_width = self.auction_width(geometry);
        if let Some((start, end)) = self.axis.opening_auction {
            if minute <= end {
                return geometry.left
                    + auction_width * minute.saturating_sub(start) as f32
                        / (end - start).max(1) as f32;
            }
            if minute < self.cash_start() {
                return geometry.left
                    + auction_width
                    + self.opening_gap() * (minute - end) as f32
                        / (self.cash_start() - end) as f32;
            }
        }
        let cash_left = geometry.left + auction_width + self.opening_gap();
        let mut elapsed = minute.saturating_sub(self.cash_start());
        let mut break_offset = 0.;
        if let Some((start, end)) = self.axis.lunch {
            if minute >= end {
                elapsed -= end - start;
                break_offset = self.lunch_gap();
            } else if minute > start {
                elapsed = start - self.cash_start();
                break_offset = self.lunch_gap() * (minute - start) as f32 / (end - start) as f32;
            }
        }
        cash_left
            + self.cash_width(geometry) * elapsed as f32 / self.cash_minutes().max(1) as f32
            + break_offset
    }

    pub(super) fn index(
        &self,
        geometry: Geometry,
        pointer: Point<Pixels>,
        bars: &[Bar],
    ) -> Option<usize> {
        if pointer.x.as_f32() < geometry.left
            || pointer.x.as_f32() > geometry.right()
            || pointer.y.as_f32() < geometry.top
            || pointer.y.as_f32() > geometry.bottom
        {
            return None;
        }
        let x = pointer.x.as_f32();
        // A matched auction is painted as a 3 px dot. Give that actual dot a
        // finite hit target even when its clock coordinate is between device pixels.
        if let Some((_, end)) = self.axis.opening_auction
            && (x - self.x(end, geometry)).abs() <= 1.5
            && let Some(index) = bars
                .iter()
                .position(|bar| market::bar_wall_minute(bar) == end)
        {
            return Some(index);
        }
        // Exclude wall-clock regions before nearest-point hit testing. In an
        // ultrawide chart a minute's hit radius can be wider than a compressed break.
        if let Some((_, end)) = self.axis.opening_auction {
            let matched = self.x(end, geometry);
            if x < matched || (x > matched && x < self.x(self.cash_start(), geometry)) {
                return None;
            }
        }
        if let Some((start, end)) = self.axis.lunch
            && x > self.x(start, geometry)
            && x < self.x(end, geometry)
        {
            return None;
        }
        if let Some((start, end)) = self.axis.closing_auction
            && x > self.x(start, geometry)
            && x < self.x(end, geometry)
        {
            return None;
        }
        let latest_x = self.x(market::bar_wall_minute(bars.last()?), geometry);
        if x > latest_x + 0.01 {
            return None;
        }
        let tolerance =
            (self.cash_width(geometry) / self.cash_minutes().max(1) as f32 * 0.51).max(0.25);
        bars.iter()
            .enumerate()
            .filter_map(|(index, bar)| {
                let distance =
                    (self.x(market::bar_wall_minute(bar), geometry) - pointer.x.as_f32()).abs();
                (distance <= tolerance).then_some((index, distance))
            })
            .min_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(index, _)| index)
    }

    pub(super) fn legend(&self) -> String {
        if self.axis.opening_auction.is_some() {
            "09:15 集合竞价 · 09:20 禁止撤单 · 09:25 撮合 · 09:30 开盘 · 11:30 / 13:00 午休 · 14:57 收盘竞价 · 15:00 收盘".into()
        } else if let Some((start, end)) = self.axis.lunch {
            format!(
                "{} 开盘 · {} / {} 午休 · {} 收盘",
                market::minute_text(self.axis.start),
                market::minute_text(start),
                market::minute_text(end),
                market::minute_text(self.axis.end)
            )
        } else {
            format!(
                "{} 开盘 · {} 收盘 · 固定全天",
                market::minute_text(self.axis.start),
                market::minute_text(self.axis.end)
            )
        }
    }

    pub(super) fn markers(&self) -> Vec<(u16, String)> {
        if self.axis.opening_auction.is_some() {
            [
                (555, "09:15"),
                (560, "09:20"),
                (565, "09:25"),
                (570, "09:30"),
                (600, "10:00"),
                (630, "10:30"),
                (660, "11:00"),
                (690, "11:30 / 13:00"),
                (810, "13:30"),
                (840, "14:00"),
                (870, "14:30"),
                (900, "15:00"),
            ]
            .into_iter()
            .map(|(minute, text)| (minute, text.into()))
            .collect()
        } else {
            let mut markers = vec![(self.axis.start, market::minute_text(self.axis.start))];
            for minute in (self.axis.start + 30..self.axis.end).step_by(60) {
                if self
                    .axis
                    .lunch
                    .is_none_or(|(start, end)| minute < start || minute > end)
                {
                    markers.push((minute, market::minute_text(minute)));
                }
            }
            if let Some((start, end)) = self.axis.lunch {
                markers.push((
                    start,
                    format!(
                        "{} / {}",
                        market::minute_text(start),
                        market::minute_text(end)
                    ),
                ));
            }
            markers.push((self.axis.end, market::minute_text(self.axis.end)));
            markers.sort_by_key(|(minute, _)| *minute);
            markers
        }
    }
}

pub(super) fn session_segments(bars: &[Bar]) -> Vec<Range<usize>> {
    let mut segments = Vec::new();
    let mut start = 0;
    for index in 1..bars.len() {
        if market::bar_wall_minute(&bars[index]) != market::bar_wall_minute(&bars[index - 1]) + 1 {
            segments.push(start..index);
            start = index;
        }
    }
    if start < bars.len() {
        segments.push(start..bars.len());
    }
    segments
}
