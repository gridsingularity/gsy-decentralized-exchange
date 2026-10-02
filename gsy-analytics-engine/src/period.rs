//! Slot and window arithmetic. All timestamps are unix seconds, UTC.

/// Start of the slot that contains `timestamp`.
pub fn align_to_slot(timestamp: i64, slot_length: i64) -> i64 {
    timestamp.div_euclid(slot_length) * slot_length
}

/// Half-open range of slots `[start, end)`, both aligned to `slot_length`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub start: i64,
    pub end: i64,
    pub slot_length: i64,
}

impl Window {
    /// The window a tick at `now` recomputes: the last `lookback_hours` of slots that ended at
    /// least `settlement_delay_minutes` ago.
    pub fn for_tick(
        now: i64,
        lookback_hours: u64,
        settlement_delay_minutes: u64,
        slot_length: i64,
    ) -> Self {
        let end = align_to_slot(now - settlement_delay_minutes as i64 * 60, slot_length);
        let start = align_to_slot(end - lookback_hours as i64 * 3600, slot_length);
        Window {
            start,
            end,
            slot_length,
        }
    }

    pub fn contains_slot(&self, slot_start: i64) -> bool {
        slot_start >= self.start && slot_start < self.end
    }

    pub fn slots(&self) -> impl Iterator<Item = i64> {
        (self.start..self.end).step_by(self.slot_length as usize)
    }
}
