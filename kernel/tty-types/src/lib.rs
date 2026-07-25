#![no_std]

use bitflags::bitflags;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Controls when a termios update takes effect.
pub enum ApplyWhen {
    /// Applies settings immediately.
    Immediate,
    /// Applies settings after pending output drains.
    Drain,
    /// Discards pending input before applying settings.
    Flush,
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct LocalFlags: u32 {
        const ICANON = 0o2;
        const ECHO = 0o10;
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Termios {
    pub input_flags: u32,
    pub output_flags: u32,
    pub control_flags: u32,
    pub local_flags: LocalFlags,
    pub line_discipline: u8,
    pub control_characters: [u8; 32],
    pub input_speed: u32,
    pub output_speed: u32,
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
