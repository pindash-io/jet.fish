// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Fast deterministic hashing (foldhash) for cache keys and maps: `SipHash`'s
//! per-call setup is wasted when a collision only costs a redundant re-layout.

use core::hash::BuildHasher;

pub use foldhash::fast::{FixedState, FoldHasher};

/// `hashbrown` map with the deterministic foldhash hasher.
pub type HashMap<K, V> = hashbrown::HashMap<K, V, FixedState>;

/// `hashbrown` set with the deterministic foldhash hasher.
pub type HashSet<K> = hashbrown::HashSet<K, FixedState>;

/// A fresh deterministic cache-key hasher.
#[must_use]
#[inline]
pub fn new_hasher() -> FoldHasher<'static> {
    FixedState::default().build_hasher()
}
