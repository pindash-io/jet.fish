// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Packed SGR attributes (fg | bg | flags in one u64).

use vtty::ColorId;
use vtty::blend_faint;
use vtty::{DEFAULT_BG_ID, DEFAULT_FG_ID, SgrFlags, UnderlineStyle};

use crate::deccara::apply_rect_flags;

/// The stored render-style bits — every [`SgrFlags`] bit except the
/// parse-only `BLINK_RAPID` (SGR 6) and `RESET` mark.
const STYLE_MASK: u64 = (SgrFlags::all().bits()
    & !(SgrFlags::BLINK_RAPID.bits() | SgrFlags::RESET.bits()))
    as u64;

const FG_SHIFT: u32 = 32;
const FG_MASK: u64 = 0xFFFF << FG_SHIFT;
const BG_SHIFT: u32 = 48;
const BG_MASK: u64 = 0xFFFF << BG_SHIFT;
const UNDERLINE_STYLE_SHIFT: u32 = 9;
const UNDERLINE_STYLE_MASK: u64 = 0b111 << UNDERLINE_STYLE_SHIFT;
const UNDERLINE_COLOR_SHIFT: u32 = 12;
const UNDERLINE_COLOR_MASK: u64 = 0xFFFF << UNDERLINE_COLOR_SHIFT;

/// Compile-time guard: the stored style bits must occupy exactly the low
/// [`UNDERLINE_STYLE_SHIFT`] bits (0..8), so [`Attrs::flags`] round-trips and
/// never collides with the underline-style field.
const _: () = assert!(
    STYLE_MASK == (1 << UNDERLINE_STYLE_SHIFT) - 1,
    "STYLE_MASK must cover exactly the underline-style bits"
);

/// Packed SGR attributes: fg | bg | flags.
///
/// Both colors are palette indices ([`ColorId`]); the default fg/bg are the
/// reserved ids 256/257.
///
/// Bit layout (the low nine style bits follow SGR-code order):
///   bit   0       : bold (SGR 1)
///   bit   1       : faint/dim (SGR 2)
///   bit   2       : italic (SGR 3)
///   bit   3       : underline (SGR 4)
///   bit   4       : blink (SGR 5/6)
///   bit   5       : inverse (SGR 7)
///   bit   6       : invisible / conceal (SGR 8)
///   bit   7       : strikethrough (SGR 9)
///   bit   8       : overline (SGR 53)
///   bits  9..11   : underline style (0 off, 1 single, 2 double, 3 curly,
///                   4 dotted, 5 dashed)
///   bits 32..47   : fg palette id
///   bits 48..63   : bg palette id
///
/// The low nine bits mirror [`SgrFlags`] bit-for-bit, so [`Attrs::flags`] is
/// a truncating cast. The parse-only `SgrFlags::BLINK_RAPID` and
/// `SgrFlags::RESET` are never stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Attrs(u64);

impl Attrs {
    /// The blank cell's attributes: default fg + default bg, no style —
    /// the single API for the reset/blank value.
    pub const DEFAULT: Self = Self(
        ((DEFAULT_FG_ID.0 as u64) << FG_SHIFT)
            | ((DEFAULT_BG_ID.0 as u64) << BG_SHIFT),
    );

    /// The style/color-metadata flags (the low nine [`SgrFlags`] bits).
    #[inline]
    pub const fn flags(self) -> SgrFlags {
        SgrFlags::from_bits_retain((self.0 & STYLE_MASK) as u16)
    }

    /// The span-level [`SgrFlags`], with `INVERSE` excluded (the cell screen
    /// resolves SGR 7 at RGB level, so a span must not re-swap).
    #[inline]
    pub const fn span_flags(self) -> SgrFlags {
        self.flags().difference(SgrFlags::INVERSE)
    }

    /// The foreground palette id.
    #[inline]
    pub const fn fg(self) -> ColorId {
        ColorId(((self.0 >> FG_SHIFT) & 0xFFFF) as u16)
    }

    /// The background palette id.
    #[inline]
    pub const fn bg(self) -> ColorId {
        ColorId(((self.0 >> BG_SHIFT) & 0xFFFF) as u16)
    }

    /// Whether SGR 1 bold is set.
    #[inline]
    pub const fn bold(self) -> bool {
        self.contains(SgrFlags::BOLD)
    }

    /// Whether SGR 2 faint/dim is set.
    #[inline]
    pub const fn dim(self) -> bool {
        self.contains(SgrFlags::FAINT)
    }

    /// Whether SGR 3 italic is set.
    #[inline]
    pub const fn italic(self) -> bool {
        self.contains(SgrFlags::ITALIC)
    }

    /// Whether underline is on.
    #[inline]
    pub const fn underline(self) -> bool {
        self.contains(SgrFlags::UNDERLINE)
    }

    /// The underline style (0 = off, 1..5 = single/double/curly/dotted/dashed).
    #[inline]
    pub const fn underline_style(self) -> UnderlineStyle {
        UnderlineStyle::from_stored(
            ((self.0 & UNDERLINE_STYLE_MASK) >> UNDERLINE_STYLE_SHIFT) as u8,
        )
    }

    /// Whether SGR 5/6 blink is set.
    #[inline]
    pub const fn blink(self) -> bool {
        self.contains(SgrFlags::BLINK)
    }

    /// Whether SGR 7 inverse video is set.
    #[inline]
    pub const fn inverse(self) -> bool {
        self.contains(SgrFlags::INVERSE)
    }

    /// Whether SGR 8 invisible (conceal) is set.
    #[inline]
    pub const fn invisible(self) -> bool {
        self.contains(SgrFlags::INVISIBLE)
    }

    /// Whether SGR 9 strikethrough is set.
    #[inline]
    pub const fn strikethrough(self) -> bool {
        self.contains(SgrFlags::STRIKETHROUGH)
    }

    /// Whether SGR 53 overline is set.
    #[inline]
    pub const fn overline(self) -> bool {
        self.contains(SgrFlags::OVERLINE)
    }

    /// The SGR 58 underline color, `None` when it follows the foreground.
    #[inline]
    pub const fn underline_color(self) -> Option<ColorId> {
        match ((self.0 & UNDERLINE_COLOR_MASK) >> UNDERLINE_COLOR_SHIFT) as u16
        {
            0 => None,
            c => Some(ColorId(c)),
        }
    }

    /// Set the foreground palette id.
    #[must_use]
    #[inline]
    pub const fn with_fg(self, fg: ColorId) -> Self {
        Self((self.0 & !FG_MASK) | ((fg.0 as u64) << FG_SHIFT))
    }

    /// Set the background palette id.
    #[must_use]
    #[inline]
    pub const fn with_bg(self, bg: ColorId) -> Self {
        Self((self.0 & !BG_MASK) | ((bg.0 as u64) << BG_SHIFT))
    }

    /// Whether every style bit in `f` is set.
    #[inline]
    pub const fn contains(self, f: SgrFlags) -> bool {
        let bits = f.bits() as u64 & STYLE_MASK;
        self.0 & bits == bits
    }

    /// Whether any style bit in `f` is set.
    #[inline]
    pub const fn intersects(self, f: SgrFlags) -> bool {
        self.0 & (f.bits() as u64 & STYLE_MASK) != 0
    }

    /// Set the style bits in `f`.
    #[must_use]
    #[inline]
    pub const fn insert(self, f: SgrFlags) -> Self {
        Self(self.0 | (f.bits() as u64 & STYLE_MASK))
    }

    /// Clear the style bits in `f`.
    #[must_use]
    #[inline]
    pub const fn remove(self, f: SgrFlags) -> Self {
        Self(self.0 & !(f.bits() as u64 & STYLE_MASK))
    }

    /// Set or clear the style bits in `f`.
    #[must_use]
    #[inline]
    pub const fn set(self, f: SgrFlags, on: bool) -> Self {
        if on { self.insert(f) } else { self.remove(f) }
    }

    /// Toggle the style bits in `f`.
    #[must_use]
    #[inline]
    pub const fn toggle(self, f: SgrFlags) -> Self {
        Self(self.0 ^ (f.bits() as u64 & STYLE_MASK))
    }

    /// Replace the nine style bits, keeping the underline style + fg/bg.
    #[inline]
    const fn with_flags(self, f: SgrFlags) -> Self {
        Self((self.0 & !STYLE_MASK) | (f.bits() as u64 & STYLE_MASK))
    }

    /// Set or clear bold.
    #[must_use]
    #[inline]
    pub fn with_bold(self, on: bool) -> Self {
        self.set(SgrFlags::BOLD, on)
    }

    /// Set or clear italic.
    #[must_use]
    #[inline]
    pub fn with_italic(self, on: bool) -> Self {
        self.set(SgrFlags::ITALIC, on)
    }

    /// Set the underline style, keeping the on/off bit in sync.
    #[must_use]
    #[inline]
    pub fn with_underline_style(self, style: UnderlineStyle) -> Self {
        Self(
            (self.set(SgrFlags::UNDERLINE, style != UnderlineStyle::None).0
                & !UNDERLINE_STYLE_MASK)
                | ((style as u64) << UNDERLINE_STYLE_SHIFT),
        )
    }

    /// Set or clear inverse video.
    #[must_use]
    #[inline]
    pub fn with_inverse(self, on: bool) -> Self {
        self.set(SgrFlags::INVERSE, on)
    }

    /// Set or clear the SGR 58 underline color (`None` = follow the foreground).
    #[must_use]
    #[inline]
    pub const fn with_underline_color(self, color: Option<ColorId>) -> Self {
        let c = match color {
            None => 0,
            Some(c) => c.0 as u64,
        };
        Self((self.0 & !UNDERLINE_COLOR_MASK) | (c << UNDERLINE_COLOR_SHIFT))
    }

    /// Set or clear a style flag; clearing `UNDERLINE` also drops its style.
    #[must_use]
    #[inline]
    pub fn apply_flags(self, f: SgrFlags, on: bool) -> Self {
        let a = self.set(f, on);
        if !on && f.contains(SgrFlags::UNDERLINE) {
            a.with_underline_style(UnderlineStyle::None)
        } else {
            a
        }
    }

    /// Apply a DECCARA/DECRARA change to the four rectangle attributes.
    /// `set` = DECCARA (set to `value`), `!set` = DECRARA (toggle).
    #[must_use]
    pub fn apply_rect_attr(
        self,
        mask: SgrFlags,
        value: SgrFlags,
        set: bool,
    ) -> Self {
        let mut f = self.flags();
        apply_rect_flags(&mut f, mask, value, set);
        self.with_flags(f)
    }
}

/// A style's raw SGR state (fg/bg/flags/underline).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Style {
    /// Foreground palette id.
    pub fg: ColorId,
    /// Background palette id.
    pub bg: ColorId,
    /// Style flags (bold / underline / inverse / blink / …).
    pub flags: SgrFlags,
    /// SGR 4:0..5 underline style.
    pub underline_style: UnderlineStyle,
    /// SGR 58 underline color id (`None` = use the foreground).
    pub underline_color: Option<ColorId>,
}

impl Style {
    /// The default style: default fg + default bg, no flags, no underline —
    /// the single API for the reset/default value.
    pub const DEFAULT: Self = Self::new(
        DEFAULT_FG_ID,
        DEFAULT_BG_ID,
        SgrFlags::empty(),
        UnderlineStyle::None,
        None,
    );

    /// Build a full style view.
    #[inline]
    pub const fn new(
        fg: ColorId,
        bg: ColorId,
        flags: SgrFlags,
        underline_style: UnderlineStyle,
        underline_color: Option<ColorId>,
    ) -> Self {
        Self { fg, bg, flags, underline_style, underline_color }
    }

    /// Build a color-only view (fg/bg/flags) with the underline fields
    /// defaulted.
    #[inline]
    pub const fn colors(fg: ColorId, bg: ColorId, flags: SgrFlags) -> Self {
        Self::new(fg, bg, flags, UnderlineStyle::None, None)
    }

    /// Set or clear a style flag; clearing `UNDERLINE` also drops its style.
    #[inline]
    #[must_use]
    pub fn apply_flags(mut self, f: SgrFlags, on: bool) -> Self {
        self.flags.set(f, on);
        if !on && f.contains(SgrFlags::UNDERLINE) {
            self.underline_style = UnderlineStyle::None;
        }
        self
    }

    /// Set the underline style and its UNDERLINE flag (mirrors
    /// [`Attrs::with_underline_style`]).
    #[inline]
    #[must_use]
    pub fn with_underline_style(mut self, style: UnderlineStyle) -> Self {
        self.flags.set(SgrFlags::UNDERLINE, style != UnderlineStyle::None);
        self.underline_style = style;
        self
    }

    /// Set the foreground palette id (mirrors [`Attrs::with_fg`]).
    #[inline]
    pub const fn with_fg(mut self, fg: ColorId) -> Self {
        self.fg = fg;
        self
    }

    /// Set the background palette id (mirrors [`Attrs::with_bg`]).
    #[inline]
    pub const fn with_bg(mut self, bg: ColorId) -> Self {
        self.bg = bg;
        self
    }

    /// Set the underline color (mirrors [`Attrs::with_underline_color`]).
    #[inline]
    pub const fn with_underline_color(
        mut self,
        color: Option<ColorId>,
    ) -> Self {
        self.underline_color = color;
        self
    }
}

impl Attrs {
    /// Project the packed pen to its raw [`Style`].
    #[must_use]
    #[inline]
    pub const fn to_style(self) -> Style {
        self.style_with(self.flags())
    }

    /// Project the pen to the span [`Style`] (like [`to_style`](Self::to_style)
    /// but with INVERSE excluded).
    #[must_use]
    #[inline]
    pub const fn to_span_style(self) -> Style {
        self.style_with(self.span_flags())
    }

    /// The shared `fg`/`bg`/`flags`/underline projection.
    const fn style_with(self, flags: SgrFlags) -> Style {
        Style::new(
            self.fg(),
            self.bg(),
            flags,
            self.underline_style(),
            self.underline_color(),
        )
    }

    /// Rebuild the pen from its raw [`Style`].
    #[must_use]
    #[inline]
    pub fn from_style(style: Style) -> Self {
        Self::DEFAULT
            .with_fg(style.fg)
            .with_bg(style.bg)
            .with_flags(style.flags)
            .with_underline_style(style.underline_style)
            .with_underline_color(style.underline_color)
    }
}

/// Resolve the effective fg/bg after faint/invisible/inverse; DECSCNM is
/// applied separately via [`reverse_colors`].
#[inline]
pub const fn apply_sgr_effects(
    fg: u32,
    bg: u32,
    dim: bool,
    invisible: bool,
    inverse: bool,
) -> (u32, u32) {
    let fg = if invisible {
        bg
    } else if dim {
        blend_faint(fg, bg)
    } else {
        fg
    };
    if inverse { (bg, fg) } else { (fg, bg) }
}

/// DECSCNM reverse video: swap only the reserved default ids, leaving
/// indexed/RGB colors untouched.
#[inline]
pub const fn reverse_colors(
    fg: ColorId,
    bg: ColorId,
    reverse: bool,
) -> (ColorId, ColorId) {
    if !reverse {
        return (fg, bg);
    }
    let fg = if fg.0 == DEFAULT_FG_ID.0 { DEFAULT_BG_ID } else { fg };
    let bg = if bg.0 == DEFAULT_BG_ID.0 { DEFAULT_FG_ID } else { bg };
    (fg, bg)
}

/// SGR 1 (bold) maps the 8 normal ANSI fg colors to their bright half
/// (id + 8) when `bright` is set.
#[inline]
#[must_use]
pub const fn bold_to_bright(fg: ColorId, bold: bool, bright: bool) -> ColorId {
    if bold && bright && fg.0 < 8 { ColorId(fg.0 + 8) } else { fg }
}

#[cfg(test)]
#[path = "../tests/attrs/mod.rs"]
mod tests;
