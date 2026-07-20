#![no_std]

extern crate alloc;

mod device;
mod file;

pub use device::TerminalDevice;
pub use file::open;
