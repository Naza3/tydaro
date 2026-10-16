use super::*;

impl ChartViewport {
    pub(super) fn range(self, len: usize) -> Range<usize> {
        if len == 0 {
            return 0..0;
        }
        let count = self.bars_visible.clamp(12.min(len), len);
        let end = len - self.offset_from_latest.min(len - count);
        end - count..end
    }

    pub(super) fn zoom(&mut self, zoom_in: bool, anchor: f32, len: usize) {
        if len == 0 {
            return;
        }
        let old = self.range(len);
        let old_count = old.len();
        let count = if zoom_in {
            (old_count as f32 * 0.8).round() as usize
        } else {
            (old_count as f32 * 1.25).ceil() as usize
        }
        .clamp(12.min(len), len);
        let anchor = anchor.clamp(0., 1.);
        let fixed = old.start as f32 + anchor * old_count as f32;
        let start = (fixed - anchor * count as f32).round().max(0.) as usize;
        let end = (start + count).min(len);
        self.bars_visible = count;
        self.offset_from_latest = len - end.max(count);
    }

    pub(super) fn reveal(&mut self, index: usize, len: usize) {
        let range = self.range(len);
        if index < range.start {
            self.offset_from_latest = len.saturating_sub(index + range.len());
        } else if index >= range.end {
            self.offset_from_latest = len.saturating_sub(index + 1);
        }
    }

    pub(super) fn pan(&mut self, bars: isize, len: usize) {
        let count = self.range(len).len();
        self.offset_from_latest = self
            .offset_from_latest
            .saturating_add_signed(bars)
            .min(len.saturating_sub(count));
    }
}
