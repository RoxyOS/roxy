#![no_std]

use bitflags::bitflags;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Controls when a terminal-attributes update takes effect.
pub enum ApplyWhen {
    /// Applies settings immediately.
    Immediate,
    /// Applies settings after pending output drains.
    Drain,
    /// Discards pending input before applying settings.
    Flush,
}

bitflags! {
    /// The terminal behavior flags a terminal applies to its input and output paths.
    ///
    /// Each flag owns one bit. The word is a field of Roxy's own terminal-attributes record
    /// rather than a word a caller shares with another personality, so it needs no base above a
    /// foreign numbering: every bit outside this type is undefined, and the syscall boundary
    /// reports it instead of dropping it.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct TerminalFlags: u32 {
        const ISIG = 1 << 0;
        const ICANON = 1 << 1;
        const ECHO = 1 << 2;
        const OPOST = 1 << 3;
        const ONLCR = 1 << 4;
        const ICRNL = 1 << 5;
        const INLCR = 1 << 6;
        const IGNCR = 1 << 7;
    }
}

/// The terminal attributes a terminal applies to its input and output paths.
///
/// Only attributes with a real effect are represented: `flags` carries the behavior flags the
/// line discipline and output path execute, and the two bytes are the only control characters a
/// terminal acts on. Attributes another personality's `termios` carries but this kernel does not
/// execute have no field here.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TerminalAttributes {
    pub flags: TerminalFlags,
    pub interrupt_byte: u8,
    pub erase_byte: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WindowSize {
    pub rows: u16,
    pub columns: u16,
    pub pixel_width: u16,
    pub pixel_height: u16,
}

impl WindowSize {
    /// A window size unavailable from the output endpoint.
    pub const UNKNOWN: Self = Self {
        rows: 0,
        columns: 0,
        pixel_width: 0,
        pixel_height: 0,
    };
}
