// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The ESC charset-designation marker (`( ) * +`).

/// The ESC charset-designation marker (ECMA-35 §8.3.8 DESIGNATE): `(` G0,
/// `)` G1, `*` G2, `+` G3.
///
/// <https://ecma-international.org/wp-content/uploads/ECMA-35_6th_edition_december_1994.pdf>
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    /// `(` — designates G0.
    LeftParen,
    /// `)` — designates G1.
    RightParen,
    /// `*` — designates G2.
    Star,
    /// `+` — designates G3.
    Plus,
}

impl Marker {
    /// The marker for a byte, or `None`.
    #[inline]
    pub const fn parse(byte: u8) -> Option<Self> {
        if byte < b'(' || byte > b'+' {
            return None;
        }
        // The markers are 0x28..=0x2B — the low 2 bits are the index.
        Some(Self::from_index((byte & 0b11) as usize))
    }

    /// The marker byte.
    #[inline]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::LeftParen => b'(',
            Self::RightParen => b')',
            Self::Star => b'*',
            Self::Plus => b'+',
        }
    }

    /// The G-set index (0..=3).
    #[inline]
    pub const fn index(self) -> usize {
        match self {
            Self::LeftParen => 0,
            Self::RightParen => 1,
            Self::Star => 2,
            Self::Plus => 3,
        }
    }

    /// The marker for a G-set index (0..=3).
    #[inline]
    pub const fn from_index(index: usize) -> Self {
        match index {
            0 => Self::LeftParen,
            1 => Self::RightParen,
            2 => Self::Star,
            _ => Self::Plus,
        }
    }
}
