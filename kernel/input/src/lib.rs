#![no_std]

/// A shared source of raw input bytes.
pub trait InputDevice: Send + Sync {
    /// Returns the oldest available byte without blocking.
    #[must_use]
    fn read_byte(&self) -> Option<u8>;
}
