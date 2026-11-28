// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Semantic command layer between the byte parser and the grid. The `Perform`
//! impl decodes dispatches into a [`Command`] and hands it to
//! [`crate::Apply::apply`]; related commands group into sub-enums ([`Move`],
//! [`Query`], [`ColorCommand`]).

use compact_str::CompactString;
use smallvec::SmallVec;

use vtty::{Charset, SgrFlags, TermFlags};

use jetfish_protocol::{CursorPos, Progress};

pub use vtty::Sgr;

pub mod codec;
mod csi;
mod dcs;
pub mod dispatch;
mod esc;
mod kitty_color;
mod marker;
mod osc;
mod sgr;
mod shell_prompt;

pub use csi::{Csi, CsiCtx};
pub use dcs::Dcs;
pub use dispatch::{CsiHandler, EscHandler, Handler, OscHandler};
pub use dispatch::{
    apply_dcs_query, dcs_query_kind, dispatch_csi, dispatch_esc, dispatch_osc,
};
pub use esc::Esc;
pub use kitty_color::apply as kitty_color_apply;
pub use kitty_color::{KittyColorKey, KittyColorOp};
pub use marker::Marker;
pub use osc::cwd_path;
pub use osc::{Osc, Osc52};
pub use sgr::{for_each_effect, resolve_color, resolve_spec};
pub use shell_prompt::ShellPrompt;

/// A semantic command decoded from an escape sequence, applied by
/// [`Apply::apply`](crate::Apply::apply).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Cursor / line movement (cell coordinates already normalized).
    Move(Move),

    /// ED (`CSI Ps J`) — erase: 0 below, 1 above, 2 all, 3 scrollback.
    EraseDisplay(u8),
    /// EL (`CSI Ps K`) — erase: 0 right, 1 left, 2 all.
    EraseLine(u8),
    /// DECSED (`CSI ? Ps J`) — selective erase display (skips protected cells).
    SelectiveEraseDisplay(u8),
    /// DECSEL (`CSI ? Ps K`) — selective erase line (skips protected cells).
    SelectiveEraseLine(u8),
    /// DECSCA (`CSI Ps " q`) — 0 off, 1/2 protect subsequently printed chars.
    Decsca(u8),
    /// DECSECH (`CSI ? Ps X`) — selective erase chars (skips protected cells).
    SelectiveEraseChars(usize),

    /// SGR (`CSI … m`) — a raw attribute sequence (params + colon bitfield).
    Sgr(Sgr),
    /// SM / RM (`CSI Ps h/l`) — set/reset a mode.
    SetMode {
        /// The mode flags to set/reset.
        mode: TermFlags,
        /// `true` = set (SM), `false` = reset (RM).
        on: bool,
    },
    /// SM / RM with multiple modes (`CSI Ps;Ps h/l`, e.g. htop's
    /// `?1006;1000h` SGR + button tracking). Each mode applies independently.
    SetModes {
        /// The modes to set/reset, one per CSI parameter.
        modes: SmallVec<TermFlags, 4>,
        /// `true` = set (SM), `false` = reset (RM).
        on: bool,
    },
    /// XTSAVE (`CSI ? Pm s`) — save the listed DEC private modes' states.
    SaveModes(Vec<i32>),
    /// XTRESTORE (`CSI ? Pm u`) — restore the saved mode states.
    RestoreModes(Vec<i32>),

    /// SU (`CSI Ps S`) — scroll the region up `n` lines.
    ScrollUp(usize),
    /// SD (`CSI Ps T`) — scroll the region down `n` lines.
    ScrollDown(usize),
    /// DECSTBM (`CSI Ps;Ps r`) — set the scroll region.
    SetScrollRegion {
        /// The top row (0-based, inclusive).
        top: usize,
        /// The bottom row (0-based, inclusive).
        bottom: usize,
    },
    /// DECSLRM — left/right margins (0-based, inclusive). `None` keeps the
    /// current margin (`CSI Pl s` / `CSI ; Pr s` set only one side).
    SetLeftRightMargins {
        /// The left margin column (0-based, inclusive), or keep.
        left: Option<usize>,
        /// The right margin column (0-based, inclusive), or keep.
        right: Option<usize>,
    },

    /// HTS — set a tab stop at the cursor.
    TabSet,
    /// TBC — 0 clears the tab at the cursor, 3 clears all.
    TabClear(u8),
    /// CTC (`CSI Ps W`) — 0/1 set, 2/3 clear current, 4 clear right, 5 clear all.
    TabControl(u8),
    /// DECST8C (`CSI ? 5 W`) — set tab stops every 8 columns.
    TabEvery8,
    /// CHT — cursor forward to the next tab stop.
    ForwardTab(usize),
    /// CBT — cursor backward to the previous tab stop.
    BackwardTab(usize),

    /// DECALN (`ESC # 8`) — screen alignment test (fill with 'E').
    AlignmentTest,

    /// ECH (`CSI Ps X`) — erase `n` chars at the cursor.
    EraseChars(usize),
    /// ICH (`CSI Ps @`) — insert `n` blank chars.
    InsertChars(usize),
    /// DCH (`CSI Ps P`) — delete `n` chars.
    DeleteChars(usize),
    /// IL (`CSI Ps L`) — insert `n` blank lines.
    InsertLines(usize),
    /// DL (`CSI Ps M`) — delete `n` lines.
    DeleteLines(usize),
    /// REP: repeat the last graphic character.
    Repeat(usize),
    /// SR (`CSI Ps SP A`): scroll the region right, blanking the left.
    ScrollRight(usize),
    /// SL (`CSI Ps SP @`): scroll the region left, blanking the right.
    ScrollLeft(usize),
    /// DECIC (`CSI Ps ' }`): insert columns at the cursor in the region.
    InsertColumns(usize),
    /// DECDC (`CSI Ps ' ~`): delete columns at the cursor in the region.
    DeleteColumns(usize),

    /// DECERA (`CSI Pt;Pl;Pb;Pr $ z`) — erase a rectangular area (spaces).
    EraseRect {
        /// Top row (1-based, inclusive).
        top: usize,
        /// Left column (1-based, inclusive).
        left: usize,
        /// Bottom row (1-based, inclusive).
        bottom: usize,
        /// Right column (1-based, inclusive).
        right: usize,
    },
    /// DECFRA (`CSI Pch;Pt;Pl;Pb;Pr $ x`) — fill a rectangular area with `ch`.
    FillRect {
        /// The fill character.
        ch: char,
        /// Top row (1-based, inclusive).
        top: usize,
        /// Left column (1-based, inclusive).
        left: usize,
        /// Bottom row (1-based, inclusive).
        bottom: usize,
        /// Right column (1-based, inclusive).
        right: usize,
    },
    /// DECCRA (`CSI Pt;Pl;Pb;Pr;Pp $ v`) — copy a rectangular area. Parsed,
    /// but a same-page copy is a no-op.
    CopyRect {
        /// Top row (1-based, inclusive).
        top: usize,
        /// Left column (1-based, inclusive).
        left: usize,
        /// Bottom row (1-based, inclusive).
        bottom: usize,
        /// Right column (1-based, inclusive).
        right: usize,
        /// Source page (0 = current, 1/2 = alternate pages).
        page: i32,
    },
    /// DECCARA / DECRARA (`CSI Pt;Pl;Pb;Pr;Ps… $ r` / `$ t`) — change or
    /// reverse the bold/underline/blink/reverse attributes in a rectangle.
    /// Coordinates are 1-based and inclusive.
    ChangeRectAttrs {
        /// Top row (1-based, inclusive).
        top: usize,
        /// Left column (1-based, inclusive).
        left: usize,
        /// Bottom row (1-based, inclusive).
        bottom: usize,
        /// Right column (1-based, inclusive).
        right: usize,
        /// `true` = DECCARA (set to `value`), `false` = DECRARA (toggle).
        set: bool,
        /// Which of the four DECCARA/DECRARA attributes to touch
        /// (bold/underline/blink/reverse as [`SgrFlags`] bits).
        mask: SgrFlags,
        /// The values to set (only `mask` bits matter; ignored when toggling).
        value: SgrFlags,
    },

    /// DECSC (`ESC 7`) — save the cursor.
    SaveCursor,
    /// `CSI s` (0 params) — ambiguous: SCO save-cursor (SCOSC) when DECLRMM
    /// (`?69`) is off, DECSLRM with default (full-width) margins when on.
    AnsiSaveCursor,
    /// DECRC (`ESC 8`) — restore the cursor.
    RestoreCursor,
    /// XTPUSHCOLORS (`CSI #P`) — push the current SGR colors on a stack.
    PushColors,
    /// XTPOPCOLORS (`CSI #Q`) — pop the SGR colors from the stack.
    PopColors,
    /// XTPREPORTCOLORS (`CSI #R`) — report the color-stack depth.
    ReportColors,
    /// DECKPAM / DECPNM (`ESC =` / `ESC >`) — application keypad mode.
    SetKeypad(bool),
    /// SCP (`CSI Ps SP k`) — set character path (0 default, 1 LTR, 2 RTL).
    /// The update-mode param is accepted and discarded; rendering is LTR-only.
    Scp(u8),

    /// Charset designation (`ESC ( 0`). `g` is the G-set index (0..=3).
    CharsetDesignate {
        /// The G-set to designate (0..=3).
        g: usize,
        /// The charset to assign.
        charset: Charset,
    },
    /// GL locking shift (ECMA-35 LS0/LS1/LS2/LS3): SI (0), SO (1), `ESC n`
    /// (2), `ESC o` (3) — which G-set the GL (0x21..=0x7E) bytes invoke.
    LockingShift(u8),

    /// RIS — full reset, content cleared.
    Reset,
    /// DECSTR — soft reset, content kept.
    SoftReset,

    /// A query that produces a `pending_reply`.
    Query(Query),

    /// DECSCUSR (`CSI Ps SP q`) — set the cursor shape.
    CursorShape(i32),
    /// `XTerm` modifyOtherKeys mode (`CSI >4;1m` / `CSI >4;0m`).
    ModifyOtherKeys(u8),
    /// XTITLEMODES (`CSI > Ps t`): 0 = hex, 1 = UTF-8, 2 = unconditional UTF-8.
    SetTitleMode(u8),
    /// OSC sequences — [vtdn.dev](https://vtdn.dev/) "OSC Sequences".
    Osc(OscCommand),

    /// Keyboard & input — [vtdn.dev](https://vtdn.dev/) "Keyboard & Input".
    /// `CSI > flags u` — kitty keyboard push flags.
    KittyKeyboardPush(u8),
    /// `CSI = flags ; mode u` — kitty progressive enhancement (set flags).
    /// `mode` 1 = assign all, 2 = set specified (OR), 3 = clear specified
    /// (AND-NOT), per the kitty keyboard protocol.
    KittyKeyboardSet {
        /// The kitty keyboard flag bitmask to apply.
        flags: u8,
        /// How to apply `flags`: 1 assign, 2 OR, 3 AND-NOT.
        mode: u8,
    },
    /// `CSI < n u` — kitty keyboard pop n levels.
    KittyKeyboardPop(usize),

    /// Graphics protocols — [vtdn.dev](https://vtdn.dev/) "Graphics Protocols".
    /// DCS q / APC _G — a kitty graphics image (header + decoded payload).
    #[cfg(feature = "kitty")]
    KittyImage(jetfish_image::KittyImage),
    /// OSC 1337;File=… — an iTerm2 inline image (decoded base64 bytes).
    #[cfg(feature = "iterm2")]
    Iterm2Image(jetfish_image::Iterm2Image),

    /// Window manipulation — [vtdn.dev](https://vtdn.dev/) "Window Manipulation".
    Window(WindowCommand),
}

/// Cursor and line movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    /// CUP / HVP (`CSI r ; c H` / `CSI r ; c f`). `0-based (col, row)`.
    To(CursorPos),
    /// CUU (`CSI Ps A`) — up, stops at the scroll-region top.
    Up(usize),
    /// CUD (`CSI Ps B`) — down, stops at the scroll-region bottom.
    Down(usize),
    /// CUF / HPR (`CSI Ps C` / `CSI Ps a`) — right.
    Right(usize),
    /// CUB (`CSI Ps D`) — left.
    Left(usize),
    /// CHA (`CSI Ps G`) — column.
    Col(usize),
    /// VPA (`CSI Ps d`) — row.
    Row(usize),
    /// CNL (`CSI Ps E`) — next line (column 0, n lines down).
    NextLine(usize),
    /// CPL (`CSI Ps F`) — previous line (column 0, n lines up).
    PrevLine(usize),
    /// NEL (`ESC E`) — carriage return + line feed (scrolls at the bottom).
    NewLine,
    /// IND (`ESC D`) — index: move down, keep column.
    Index,
    /// RI (`ESC M`) — reverse index: move up, keep column.
    ReverseIndex,
}

/// A query that produces a `pending_reply`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    /// DSR (`CSI Ps n`) — device status report.
    DeviceStatus(u8),
    /// DECXCPR (`CSI ? 6 n`) — extended cursor position report (`CSI ?r;cR`).
    ExtendedCursorPosition,
    /// DA (`CSI c` / `CSI > c`) — device attributes.
    DeviceAttrs(u8),
    /// DECRQM (`CSI Ps$p` / `CSI ?Ps$p`) — report a mode's state.
    Mode {
        /// The mode number to query.
        param: i32,
        /// `true` for the private (`?`) form.
        private: bool,
    },
    /// DECRQSS (`DCS $ q <setting> ST`) — report a setting.
    Setting(Vec<u8>),
    /// XTGETTCAP (`DCS + q <hex cap> ST`) — report a terminfo capability.
    Termcap(Vec<u8>),
    /// XTVERSION (`CSI > q`) — report the version.
    Version,
    /// Kitty keyboard (`CSI ? u`) — report the active flags.
    KittyKeyboard,
    /// modifyOtherKeys (`CSI ? 4 m`) — report the mode.
    ModifyOtherKeys,
    /// XTWINOPS (`CSI 14/16/18 t`) — report window/text-area size.
    WindowSize {
        /// The report code (14 = pixels, 16 = cells, 18 = chars).
        reply_code: u8,
    },
    /// XTWINOPS (`CSI 13 t`) — report the window position in pixels.
    WindowPosition,
    /// XTWINOPS (`CSI 11 t`) — report the window state (1 = normal).
    WindowState,
    /// OSC 10/11/12 `?` — default-color query.
    DefaultColor(DefaultColor),
    /// OSC 4 `?` — palette-entry color query.
    PaletteColor(usize),
    /// `CSI ?996n` — color-scheme report query (XTCOLORMOD).
    ColorScheme,
    /// OSC 52 `?` — clipboard read query (selector `c`/`p`/`s`).
    Clipboard(u8),
    /// DECRQCRA (`CSI Pi;Pg;Pt;Pl;Pb;Pr * y`) — checksum a rectangular area.
    /// Coordinates are 1-based inclusive; the page number isn't carried.
    ///
    /// NOTE: this is an exfiltration oracle — checksum one cell at a time to
    /// read back the screen (vte dummy-replies `0000`). Jetfish replies with
    /// the real checksum like xterm/wezterm for conformance; gate it behind a
    /// config flag if undesirable.
    ChecksumRect {
        /// The request id, echoed verbatim in the reply.
        id: i32,
        /// Top row (1-based, inclusive).
        top: usize,
        /// Left column (1-based, inclusive).
        left: usize,
        /// Bottom row (1-based, inclusive).
        bottom: usize,
        /// Right column (1-based, inclusive).
        right: usize,
    },
}

/// A default color target for OSC 10/11/12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum DefaultColor {
    /// The default foreground (`OSC 10`).
    Foreground,
    /// The default background (`OSC 11`).
    Background,
    /// The default cursor color (`OSC 12`).
    Cursor,
}

impl DefaultColor {
    /// The OSC code that SETS this default color (10/11/12).
    pub const fn as_set(self) -> u16 {
        match self {
            Self::Foreground => 10,
            Self::Background => 11,
            Self::Cursor => 12,
        }
    }

    /// The OSC code that RESETS this default color, as a byte string
    /// (110/111/112) — the codec passes it straight to the OSC params.
    pub const fn as_reset(self) -> &'static [u8] {
        match self {
            Self::Foreground => b"110",
            Self::Background => b"111",
            Self::Cursor => b"112",
        }
    }

    /// The OSC code that SETS this color's highlight (17 = background,
    /// 19 = foreground). The cursor has no highlight and reuses 12.
    pub const fn as_highlight_set(self) -> u16 {
        match self {
            Self::Background => 17,
            Self::Foreground => 19,
            // Unreachable from decode (OSC 17/19 only); kept total.
            Self::Cursor => 12,
        }
    }

    /// The OSC code that RESETS this color's highlight, as a byte string
    /// (117/119). The cursor has no highlight and reuses 112.
    pub const fn as_highlight_reset(self) -> &'static [u8] {
        match self {
            Self::Background => b"117",
            Self::Foreground => b"119",
            Self::Cursor => b"112",
        }
    }
}

/// A color mutation — the OSC color commands grouped under
/// [`OscCommand::Color`] (OSC 4/10/11/12/17/19/104/110-112/117/119).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorCommand {
    /// OSC 10/11/12 — set a default color.
    SetDefault {
        /// Which default (fg/bg/cursor).
        target: DefaultColor,
        /// The 0xRRGGBBAA color.
        rgb: u32,
    },
    /// OSC 110/111/112 — reset a default color.
    ResetDefault(DefaultColor),
    /// OSC 17 (highlight background) / OSC 19 (highlight foreground).
    SetHighlight {
        /// Which highlight color (bg/fg).
        target: DefaultColor,
        /// The 0xRRGGBBAA color.
        rgb: u32,
    },
    /// OSC 117 (highlight background) / OSC 119 (highlight foreground).
    ResetHighlight(DefaultColor),
    /// OSC 4 — set a palette entry.
    SetPalette {
        /// The palette index (0..=255).
        index: usize,
        /// The 0xRRGGBBAA color.
        rgb: u32,
    },
    /// OSC 4 — set several palette entries (Gogh/WezTerm multi-entry form).
    SetPaletteBatch(Vec<(usize, u32)>),
    /// OSC 104 — reset palette entries (`None` = all).
    ResetPalette(Option<usize>),
}

/// An OSC 8 hyperlink: the URI plus the optional `id=` grouping parameter
/// (used for link nesting). Inline [`CompactString`]s — URIs are short.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hyperlink {
    /// The link target.
    pub uri: CompactString,
    /// The `id=` parameter, if any (groups overlapping links).
    pub id: Option<CompactString>,
}

/// OSC sequences — [vtdn.dev](https://vtdn.dev/) "OSC Sequences".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OscCommand {
    /// OSC 0/1/2 — set the window title.
    SetTitle(CompactString),
    /// OSC 52 — store a base64 payload in the clipboard.
    SetClipboard(CompactString),
    /// `ConEmu` / Windows Terminal progress (`OSC 9;4;state[;percent]`).
    Progress(Progress),
    /// `ConEmu` tab title (`OSC 9;3;<title>` set, `OSC 9;3` reset).
    SetTabTitle(Option<CompactString>),
    /// OSC 9 / OSC 777 — a desktop notification request.
    Notify(CompactString),
    /// OSC 8 — open/close a hyperlink (`Some` = open, `None` = close).
    SetHyperlink(Option<Hyperlink>),
    /// OSC 7 — report the current working directory.
    SetCwd(CompactString),
    /// OSC 133 — shell-integration marker (action + options).
    ShellIntegration(ShellPrompt),
    /// A color mutation (OSC 4/5/6/10/11/12/17/19/104/110-117).
    Color(ColorCommand),
    /// OSC 99 / OSC 777 — a desktop notification request (kitty / urxvt).
    Notification(crate::notification::Notification),
    /// OSC 22 — set the mouse cursor icon name.
    MouseCursor(CompactString),
    /// OSC 50 — set the text cursor shape (`CursorShape=N`).
    CursorShape(i32),
    /// OSC 21 — Kitty Color Protocol: query/set/reset colors by key.
    KittyColor(Vec<KittyColorOp>),
}

/// Window manipulation commands — [vtdn.dev](https://vtdn.dev/) "Window Manipulation".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowCommand {
    /// `CSI 8;rows;cols t` — the app requests a resize to `rows`×`cols` cells.
    ResizeTextArea {
        /// The requested row count.
        rows: usize,
        /// The requested column count.
        cols: usize,
    },
    /// `CSI 22 t` — push the current window title onto the stack.
    PushTitle,
    /// `CSI 23 t` — pop and restore the window title.
    PopTitle,
}

impl From<OscCommand> for Command {
    fn from(value: OscCommand) -> Self {
        Self::Osc(value)
    }
}

impl From<WindowCommand> for Command {
    fn from(value: WindowCommand) -> Self {
        Self::Window(value)
    }
}

impl From<Move> for Command {
    fn from(value: Move) -> Self {
        Self::Move(value)
    }
}

impl From<Query> for Command {
    fn from(value: Query) -> Self {
        Self::Query(value)
    }
}

impl From<ColorCommand> for Command {
    fn from(value: ColorCommand) -> Self {
        Self::Osc(OscCommand::Color(value))
    }
}
