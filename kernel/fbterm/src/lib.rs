#![no_std]

extern crate alloc;

mod adapter;
mod ansi;
mod color;
mod console;
mod framebuffer;
mod renderer;
mod screen;

use alloc::sync::Arc;
use roxy_boot::BootInfo;
use roxy_terminal::TerminalOutput;
use spin::Once;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitError {
    NoFramebuffer,
    UnsupportedMode,
    InvalidLayout,
}

static TERMINAL: Once<Arc<adapter::FbTerminal>> = Once::new();

/// Initializes the framebuffer terminal once.
///
/// # Errors
///
/// Returns an error when no framebuffer exists or its mode and layout are unsupported.
///
/// # Panics
///
/// Panics when the framebuffer terminal has already been initialized successfully.
pub fn initialize(boot_info: &BootInfo) -> Result<(), InitError> {
    assert!(
        TERMINAL.get().is_none(),
        "framebuffer terminal was already initialized"
    );

    let terminal = adapter::FbTerminal::new(&boot_info.framebuffers)?;

    TERMINAL.call_once(|| Arc::new(terminal));

    Ok(())
}

/// Returns the initialized framebuffer terminal, if the selected mode is supported.
#[must_use]
pub fn terminal() -> Option<Arc<dyn TerminalOutput>> {
    TERMINAL
        .get()
        .map(|terminal| terminal.clone() as Arc<dyn TerminalOutput>)
}
