#![no_std]

extern crate alloc;

mod file;

use alloc::sync::Arc;

use roxy_fd::OpenFile;
use roxy_input::InputDevice;
use roxy_terminal::TerminalOutput;
use spin::Once;

pub use file::open_file;

struct Tty {
    input: Arc<dyn InputDevice>,
    output: Arc<dyn TerminalOutput>,
}

static TTY: Once<Tty> = Once::new();

/// Publishes the one TTY used for initial process descriptors.
///
/// # Panics
///
/// Panics when called more than once.
pub fn initialize(input: Arc<dyn InputDevice>, output: Arc<dyn TerminalOutput>) {
    assert!(TTY.get().is_none(), "TTY initialized twice");
    TTY.call_once(|| Tty { input, output });
}

/// Opens an independent descriptor for the initialized TTY.
///
/// # Panics
///
/// Panics when the TTY has not been initialized.
#[must_use]
pub fn open() -> Arc<OpenFile> {
    let tty = TTY.get().expect("TTY must be initialized before opening");

    open_file(tty.input.clone(), tty.output.clone())
}
