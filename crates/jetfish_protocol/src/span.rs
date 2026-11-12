// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! SGR color spans over byte offsets, plus span-style lookups.

use core::range::Range;
use std::ops::{Deref, DerefMut};

use crate::blink::blink_on;
use crate::{ColorId, SgrFlags, Style, StyleFlags, UnderlineStyle};

/// A styled text range over `state.text` byte offsets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    /// Byte range of the span in `state.text` (half-open `start..end`), so a
    /// line's styled slice is `&text[span.range]`.
    pub range: Range<usize>,
    /// The raw SGR style (fg / bg / flags / underline style + color).
    pub style: Style,
}

impl Span {
    /// Construct a span.
    pub const fn new(range: Range<usize>, style: Style) -> Self {
        Self { range, style }
    }

    /// Whether `byte` falls inside this span's half-open `[start, end)` range.
    #[inline]
    pub fn contains(&self, byte: usize) -> bool {
        self.range.contains(&byte)
    }

    /// Effective foreground: SGR 5 blink paints the background while the
    /// blink phase is OFF. `blink` is the caller's phase (kept `const`/pure).
    #[inline]
    pub const fn effective_fg(&self, blink: bool) -> ColorId {
        if self.style.flags.contains(SgrFlags::BLINK) && !blink {
            self.style.bg
        } else {
            self.style.fg
        }
    }
}

/// The screen's sorted SGR color spans: a `Vec<Span>` wrapper that adds
/// byte-addressed lookups and owns the sorted-by-start invariant.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spans(Vec<Span>);

impl Spans {
    /// A fresh empty span list.
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// The span covering `byte` (`None` in a gap). Sorted by start, so this
    /// is a binary search.
    #[inline]
    pub fn span_at(&self, byte: usize) -> Option<&Span> {
        // `partition_point` returns the index of the first span starting after
        // `byte`; the covering span, if any, is the one immediately before it.
        let idx = self.partition_point(|sp| sp.range.start <= byte);
        let sp = self.get(idx.checked_sub(1)?)?;
        sp.contains(byte).then_some(sp)
    }

    /// Background palette id for a byte offset.
    #[inline]
    pub fn bg_at(&self, byte: usize, default_bg: ColorId) -> ColorId {
        self.span_at(byte).map_or(default_bg, |sp| sp.style.bg)
    }

    /// Foreground palette id + decoration flags for a byte offset.
    #[inline]
    pub fn style_at(
        &self,
        byte: usize,
        default_fg: ColorId,
    ) -> (ColorId, StyleFlags) {
        match self.span_at(byte) {
            Some(sp) => {
                (sp.effective_fg(blink_on()), sp.style.flags.decorations())
            }
            None => (default_fg, StyleFlags::empty()),
        }
    }

    /// Full `SgrFlags` for a byte offset (including INVERSE/BLINK, which
    /// [`Spans::style_at`]'s decoration subset drops).
    #[inline]
    pub fn flags_at(&self, byte: usize) -> SgrFlags {
        self.span_at(byte).map_or(SgrFlags::empty(), |sp| sp.style.flags)
    }

    /// Underline style + color id for a byte offset. The color is `None` for
    /// the default (foreground) underline color.
    #[inline]
    pub fn underline_at(
        &self,
        byte: usize,
    ) -> (UnderlineStyle, Option<ColorId>) {
        self.span_at(byte).map_or((UnderlineStyle::None, None), |sp| {
            (sp.style.underline_style, sp.style.underline_color)
        })
    }

    /// The spans overlapping `[start, end)`: a binary-searched window over
    /// the sorted spans.
    #[inline]
    pub fn overlapping(
        &self,
        start: usize,
        end: usize,
    ) -> impl Iterator<Item = &Span> {
        let first = self.partition_point(|sp| sp.range.end <= start);
        self[first..].iter().take_while(move |sp| sp.range.start < end)
    }

    /// Hash the overlapping spans, so a color/style-only change is detected.
    pub fn hash_for_range(&self, start: usize, end: usize) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = crate::new_hasher();
        for sp in self.overlapping(start, end) {
            sp.range.start.hash(&mut h);
            sp.range.end.hash(&mut h);
            sp.style.fg.hash(&mut h);
            sp.style.bg.hash(&mut h);
            sp.style.flags.bits().hash(&mut h);
            sp.style.underline_style.hash(&mut h);
            sp.style.underline_color.hash(&mut h);
        }
        h.finish()
    }

    /// Hash only the shaping-relevant properties (boundaries + bold/italic/
    /// dim), so a color-only change reuses the shaped layout.
    pub fn shape_hash_for_range(&self, start: usize, end: usize) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = crate::new_hasher();
        for sp in self.overlapping(start, end) {
            sp.range.start.hash(&mut h);
            sp.range.end.hash(&mut h);
            sp.style.flags.contains(SgrFlags::BOLD).hash(&mut h);
            sp.style.flags.contains(SgrFlags::ITALIC).hash(&mut h);
            sp.style.flags.contains(SgrFlags::FAINT).hash(&mut h);
        }
        h.finish()
    }
}

impl From<Vec<Span>> for Spans {
    #[inline]
    fn from(spans: Vec<Span>) -> Self {
        Self(spans)
    }
}

impl PartialEq<Vec<Span>> for Spans {
    #[inline]
    fn eq(&self, other: &Vec<Span>) -> bool {
        &self.0 == other
    }
}

impl PartialEq<Spans> for Vec<Span> {
    #[inline]
    fn eq(&self, other: &Spans) -> bool {
        self == &other.0
    }
}

impl<'a> IntoIterator for &'a Spans {
    type Item = &'a Span;
    type IntoIter = std::slice::Iter<'a, Span>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut Spans {
    type Item = &'a mut Span;
    type IntoIter = std::slice::IterMut<'a, Span>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl Deref for Spans {
    type Target = Vec<Span>;

    #[inline]
    fn deref(&self) -> &Vec<Span> {
        &self.0
    }
}

impl DerefMut for Spans {
    #[inline]
    fn deref_mut(&mut self) -> &mut Vec<Span> {
        &mut self.0
    }
}
