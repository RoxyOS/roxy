#![no_std]

extern crate alloc;

mod controlling;
mod core;
mod input;
mod ioctl;
mod output;
#[cfg(feature = "kernel-test")]
mod test_support;

pub use controlling::{ControlTerminal, ControllingTerminalResolver};
pub use core::{TtyCore, controlling_terminal_of};
pub use input::TerminalInputSource;
pub use output::{OutputError, TtyOutput};
