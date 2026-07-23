#![no_std]

extern crate alloc;

mod encoder;
mod file;
mod tty;

use alloc::sync::Arc;

use roxy_fd::OpenFile;
use roxy_input::InputDevice;
use roxy_line_discipline::LineDiscipline;
use roxy_terminal::TerminalOutput;
use roxy_utils::Lock;
use spin::Once;

use file::TtyFile;

struct Tty {
    input: Arc<dyn InputDevice>,
    output: Arc<dyn TerminalOutput>,
    line_discipline: Lock<LineDiscipline>,
    // Holds encoded event bytes that did not fit in the previous read buffer.
    pending: Lock<Option<encoder::EncodedInputEvent>>,
    pending_offset: Lock<usize>,
    pending_result: Lock<Option<roxy_line_discipline::ProcessResult>>,
    read_lock: Lock<()>,
}

static TTY: Once<Arc<Tty>> = Once::new();

/// Publishes the one TTY used for initial process descriptors.
///
/// # Panics
///
/// Panics when called more than once.
pub fn initialize(input: Arc<dyn InputDevice>, output: Arc<dyn TerminalOutput>) {
    assert!(TTY.get().is_none(), "TTY initialized twice");
    TTY.call_once(|| Arc::new(Tty::new(input, output)));
}

/// Opens an independent descriptor for the initialized TTY.
///
/// # Panics
///
/// Panics when the TTY has not been initialized.
#[must_use]
pub fn open() -> Arc<OpenFile> {
    let tty = TTY.get().expect("TTY must be initialized before opening");

    TtyFile::open(tty.clone())
}
