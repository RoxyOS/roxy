#![no_std]

#[cfg(not(target_arch = "x86_64"))]
compile_error!("roxy-ps2 currently supports only x86_64 i8042 controllers");

mod decoder;
mod i8042;
mod input;

use core::sync::atomic::{AtomicBool, Ordering};

use roxy_arch::{Architecture, CurrentArchitectureBackend, IrqLine};
use roxy_utils::Lock;

use i8042::I8042FirstPort;
use input::KeyboardInput;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static KEYBOARD_INPUT: Lock<KeyboardInput> = Lock::new(KeyboardInput::new());

/// Initializes the i8042 first port and registers its ISA IRQ1 handler.
///
/// # Panics
///
/// Panics when the controller cannot be configured or the keyboard handshake times out.
pub fn initialize() {
    assert!(
        !INITIALIZED.swap(true, Ordering::AcqRel),
        "PS/2 keyboard initialized twice"
    );
    I8042FirstPort::initialize().expect("initialize PS/2 keyboard controller");
    let irq = IrqLine::new(1).expect("ISA IRQ1 must be available");
    roxy_interrupt::register_irq_handler(irq, handle_irq);
    roxy_interrupt::unmask_irq(irq);
}

/// Returns the oldest buffered ASCII key press without blocking.
#[must_use]
pub fn read() -> Option<u8> {
    CurrentArchitectureBackend::without_interrupts(|| KEYBOARD_INPUT.lock().read())
}

fn handle_irq() {
    let scancode = I8042FirstPort::read_data();
    KEYBOARD_INPUT.lock().process_scancode(scancode);
}

#[cfg(feature = "kernel-test")]
pub fn inject_for_test(input_bytes: &[u8]) {
    CurrentArchitectureBackend::without_interrupts(|| {
        let mut input = KEYBOARD_INPUT.lock();
        for &byte in input_bytes {
            input.enqueue_byte(byte);
        }
    });
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    kernel_test!("roxy-ps2::empty-read", empty_read, {
        assert_eq!(super::read(), None);
    });
}
