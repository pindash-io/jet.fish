// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Horizontal tab stops, shared by the line and cell screens.

/// A sorted set of 0-based horizontal tab-stop columns, shared by both
/// screens. Backed by an inline [`smallvec::SmallVec`].
#[derive(Debug, Clone, Default)]
pub struct TabStops(smallvec::SmallVec<usize, 8>);

impl core::ops::Deref for TabStops {
    type Target = [usize];

    #[inline]
    fn deref(&self) -> &[usize] {
        &self.0
    }
}

impl core::ops::DerefMut for TabStops {
    #[inline]
    fn deref_mut(&mut self) -> &mut [usize] {
        &mut self.0
    }
}

impl TabStops {
    /// Power-on default: every 8 columns, first at 8 (never 0).
    pub fn default(cols: usize) -> Self {
        Self((8..cols).step_by(8).collect())
    }

    /// HTS / CTC-set: add a stop at 0-based `pos` (idempotent, stays sorted).
    pub fn set(&mut self, pos: usize) {
        if self.0.contains(&pos) {
            return;
        }
        self.0.push(pos);
        self.0.sort_unstable();
    }

    /// TBC 0 / CTC-clear: remove the stop at 0-based `pos`.
    pub fn remove(&mut self, pos: usize) {
        // `partition_point` locates the stop in the sorted set, matching
        // `next`/`prev`; a miss returns fast.
        let i = self.0.partition_point(|&s| s < pos);
        if i >= self.0.len() || self.0[i] != pos {
            return;
        }
        self.0.remove(i);
    }

    /// TBC 1 / CTC 4: drop every stop at/right of 0-based `pos`.
    pub fn remove_from(&mut self, pos: usize) {
        self.0.retain(|s| *s < pos);
    }

    /// TBC 3 / CTC 5: drop every stop.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// TBC (`CSI Ps g`) at 0-based `pos`: 0 clears the stop, 3 clears all.
    pub fn tab_clear(&mut self, mode: u8, pos: usize) {
        match mode {
            0 => self.remove(pos),
            3 => self.clear(),
            _ => {}
        }
    }

    /// CTC (`CSI Ps W`) at the 0-based cursor `pos`: 0/1 set, 2/3 clear
    /// current, 4 clear to the right, 5 clear all (ECMA-48).
    pub fn tab_control(&mut self, mode: u8, pos: usize) {
        match mode {
            0 | 1 => self.set(pos),
            2 | 3 => self.remove(pos),
            4 => self.remove_from(pos + 1),
            5 => self.clear(),
            _ => {}
        }
    }

    /// The first stop strictly after 0-based `pos`, or `None`.
    #[inline]
    pub fn next(&self, pos: usize) -> Option<usize> {
        let i = self.0.partition_point(|&s| s <= pos);
        self.0.get(i).copied()
    }

    /// The last stop strictly before 0-based `pos`, or `None`.
    #[inline]
    pub fn prev(&self, pos: usize) -> Option<usize> {
        let i = self.0.partition_point(|&s| s < pos);
        if i == 0 { None } else { Some(self.0[i - 1]) }
    }

    /// The next stop strictly after `pos`, clamped to `limit`; `None` when the
    /// cursor is already at/right of `limit` (CHT stops there). Shared by both
    /// screens' `forward_tab` — the cell grid calls it with its 0-based right
    /// margin, the line screen with its resolved 0-based right margin.
    #[inline]
    pub fn next_clamped(&self, pos: usize, limit: usize) -> Option<usize> {
        (pos < limit).then(|| self.next(pos).map_or(limit, |s| s.min(limit)))
    }

    /// The last stop strictly before `pos`, clamped to `limit`; `None` when the
    /// cursor is already at/left of `limit` (CBT stops there). Shared by both
    /// screens' `backward_tab`.
    #[inline]
    pub fn prev_clamped(&self, pos: usize, limit: usize) -> Option<usize> {
        (pos > limit).then(|| self.prev(pos).map_or(limit, |s| s.max(limit)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_clamped_stops_at_the_limit() {
        let tabs = TabStops::default(40); // stops at 8, 16, 24, 32
        // A stop inside the limit.
        assert_eq!(tabs.next_clamped(0, 30), Some(8));
        // No stop after `pos` but inside the limit → the limit.
        assert_eq!(tabs.next_clamped(33, 34), Some(34));
        // A stop beyond the limit is clamped to it.
        assert_eq!(tabs.next_clamped(20, 25), Some(24));
        // At/right of the limit → None (CHT stops).
        assert_eq!(tabs.next_clamped(25, 25), None);
    }

    #[test]
    fn prev_clamped_stops_at_the_limit() {
        let tabs = TabStops::default(40); // stops at 8, 16, 24, 32
        // A stop inside the limit.
        assert_eq!(tabs.prev_clamped(20, 4), Some(16));
        // No stop before `pos` but inside the limit → the limit.
        assert_eq!(tabs.prev_clamped(4, 2), Some(2));
        // At/left of the limit → None (CBT stops).
        assert_eq!(tabs.prev_clamped(2, 2), None);
    }
}
