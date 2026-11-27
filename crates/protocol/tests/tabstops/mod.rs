// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

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
