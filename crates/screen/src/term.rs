// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The default terminal flags.

use vtty::TermFlags;

/// Default terminal flags (matching the cell screen's power-on modes: cursor
/// visible, autowrap, and cursor blink).
pub const DEFAULT_FLAGS: TermFlags = TermFlags::CURSOR_VISIBLE
    .union(TermFlags::WRAPAROUND)
    .union(TermFlags::CURSOR_BLINK);
