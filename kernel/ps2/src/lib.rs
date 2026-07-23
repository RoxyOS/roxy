#![no_std]

extern crate alloc;

#[cfg(not(target_arch = "x86_64"))]
compile_error!("roxy-ps2 currently supports only x86_64 i8042 controllers");

mod decoder;
mod i8042;
mod input;

use core::sync::atomic::{AtomicBool, Ordering};

use alloc::sync::Arc;

use roxy_arch::{Architecture, CurrentArchitectureBackend, IrqLine};
use roxy_input::InputDevice;
use roxy_utils::Lock;
use spin::Once;

use i8042::I8042FirstPort;
use input::KeyboardInput;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static KEYBOARD_INPUT: Lock<KeyboardInput> = Lock::new(KeyboardInput::new());
static INPUT_DEVICE: Once<Arc<Ps2InputDevice>> = Once::new();

struct Ps2InputDevice;

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
    INPUT_DEVICE.call_once(|| Arc::new(Ps2InputDevice));
}

/// Returns the initialized PS/2 input device.
///
/// # Panics
///
/// Panics when the PS/2 driver has not been initialized.
#[must_use]
pub fn input_device() -> Arc<dyn InputDevice> {
    INPUT_DEVICE
        .get()
        .expect("PS/2 input device must be initialized before use")
        .clone()
}

fn handle_irq() {
    let scancode = I8042FirstPort::read_data();
    KEYBOARD_INPUT.lock().process_scancode(scancode);
}

impl InputDevice for Ps2InputDevice {
    fn read_event(&self) -> Option<roxy_input::InputEvent> {
        CurrentArchitectureBackend::without_interrupts(|| KEYBOARD_INPUT.lock().read())
    }
}

#[cfg(feature = "kernel-test")]
pub fn inject_for_test(input_bytes: &[u8]) {
    CurrentArchitectureBackend::without_interrupts(|| {
        let mut input = KEYBOARD_INPUT.lock();

        for &byte in input_bytes {
            input.enqueue_event(roxy_input::InputEvent::Character(char::from(byte)));
        }
    });
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    kernel_test!("roxy-ps2::input-device", reads_injected_bytes, {
        super::inject_for_test(b"ok");

        assert_eq!(
            super::input_device().read_event(),
            Some(roxy_input::InputEvent::Character('o'))
        );
        assert_eq!(
            super::input_device().read_event(),
            Some(roxy_input::InputEvent::Character('k'))
        );
        assert_eq!(super::input_device().read_event(), None);
    });
}
