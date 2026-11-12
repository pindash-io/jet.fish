// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Fast deterministic hashing (foldhash) for cache keys and maps. `std`'s
//! `SipHash` setup is per-call overhead; a collision only causes a redundant
//! re-layout. A fixed seed keeps hashes reproducible across runs.

use core::hash::BuildHasher;

pub use foldhash::fast::{FixedState, FoldHasher};

/// A fixed seed so cache keys are deterministic across processes.
const FIXED_HASHER: FixedState = FixedState::with_seed(
    0b1001010111101110000001001100010000000011001001101011001001111000,
);

/// Deterministic `BuildHasher` (random-but-fixed state).
#[derive(Copy, Clone, Default, Debug)]
pub struct FixedHasher;

impl BuildHasher for FixedHasher {
    type Hasher = FoldHasher<'static>;

    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        FIXED_HASHER.build_hasher()
    }
}

/// A fresh deterministic cache-key hasher.
#[inline]
pub fn new_hasher() -> FoldHasher<'static> {
    FIXED_HASHER.build_hasher()
}

/// `hashbrown` map with the deterministic foldhash hasher.
pub type HashMap<K, V> = hashbrown::HashMap<K, V, FixedHasher>;

/// `hashbrown` set with the deterministic foldhash hasher.
pub type HashSet<K> = hashbrown::HashSet<K, FixedHasher>;
