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

pub use framebuffer::{ColorChannelLayout, FramebufferLayout};

static TERMINAL: Once<Arc<adapter::FbTerminal>> = Once::new();
static LAYOUT: Once<FramebufferLayout> = Once::new();

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
    let mut layout = terminal.layout();

    // The boot protocol reports the framebuffer through its HHDM mapping, but the layout is
    // the userspace-facing contract (smem_start, mmap targets), so publish the physical
    // address. The terminal keeps the virtual address for its own drawing.
    layout.address = layout
        .address
        .checked_sub(boot_info.hhdm_offset)
        .ok_or(InitError::InvalidLayout)?;

    TERMINAL.call_once(|| Arc::new(terminal));
    LAYOUT.call_once(|| layout);

    Ok(())
}

/// Returns the initialized framebuffer terminal, if the selected mode is supported.
#[must_use]
pub fn terminal() -> Option<Arc<dyn TerminalOutput>> {
    TERMINAL
        .get()
        .map(|terminal| terminal.clone() as Arc<dyn TerminalOutput>)
}

/// Returns the validated framebuffer layout, when the framebuffer terminal initialized it.
#[must_use]
pub fn framebuffer_layout() -> Option<&'static FramebufferLayout> {
    LAYOUT.get()
}
