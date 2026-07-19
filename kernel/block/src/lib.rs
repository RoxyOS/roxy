#![no_std]
#![allow(clippy::missing_errors_doc)]

extern crate alloc;

mod devices;

use core::fmt;

pub use devices::RamDisk;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockError {
    OutOfBounds,
    Misaligned,
    Io,
    Unsupported,
}

impl fmt::Display for BlockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OutOfBounds => "block range is out of bounds",
            Self::Misaligned => "block I/O is misaligned",
            Self::Io => "block device I/O failed",
            Self::Unsupported => "block operation is unsupported",
        })
    }
}

impl core::error::Error for BlockError {}

pub trait BlockDevice: Send + Sync {
    fn block_size(&self) -> usize;
    fn block_count(&self) -> u64;
    fn read_blocks(&self, start: u64, destination: &mut [u8]) -> Result<(), BlockError>;
    fn write_blocks(&self, start: u64, source: &[u8]) -> Result<(), BlockError>;
    fn flush(&self) -> Result<(), BlockError>;
}
