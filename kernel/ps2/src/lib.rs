#![no_std]

extern crate alloc;

#[cfg(not(target_arch = "x86_64"))]
compile_error!("roxy-ps2 currently supports only x86_64 i8042 controllers");

mod i8042;
mod mouse;
mod psaux;
mod scancode;

use core::sync::atomic::{AtomicBool, Ordering};

use roxy_arch::IrqLine;
use roxy_utils::Lock;

use i8042::{I8042FirstPort, I8042SecondPort};
use scancode::ScancodeParser;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static KEYBOARD_PARSER: Lock<ScancodeParser> = Lock::new(ScancodeParser::new());

/// Initializes the i8042 first port (keyboard) and second port (mouse), and registers the ISA
/// IRQ1/IRQ12 handlers.
///
/// # Panics
///
/// Panics when the controller cannot be configured or the keyboard handshake times out. A
/// missing or failed mouse is tolerated and only logs a message.
pub fn initialize() {
    assert!(
        !INITIALIZED.swap(true, Ordering::AcqRel),
        "PS/2 input initialized twice"
    );
    I8042FirstPort::initialize().expect("initialize PS/2 keyboard controller");
    psaux::initialize_poll_listeners();
    match I8042SecondPort::initialize() {
        Ok(()) => {
            let irq = IrqLine::new(12).expect("ISA IRQ12 must be available");
            roxy_interrupt::register_irq_handler(irq, handle_mouse_irq);
            roxy_interrupt::unmask_irq(irq);
        }
        Err(error) => {
            roxy_serial::e_println!("PS/2 mouse unavailable: {error:?}");
        }
    }
    let irq = IrqLine::new(1).expect("ISA IRQ1 must be available");
    roxy_interrupt::register_irq_handler(irq, handle_irq);
    roxy_interrupt::unmask_irq(irq);
}

/// Registers `/dev/psaux` with the shared device registry.
///
/// The node is present even when no mouse is attached; reads simply stay empty until the mouse
/// delivers bytes.
///
/// # Panics
///
/// Panics when the PS/2 driver has not been initialized or `psaux` is already registered.
pub fn register_psaux(registry: &roxy_devfs::DeviceRegistry) {
    assert!(
        INITIALIZED.load(Ordering::Acquire),
        "PS/2 input not initialized"
    );
    psaux::register_psaux(registry);
}

fn handle_irq() {
    let scancode = I8042FirstPort::read_data();
    let Some(event) = KEYBOARD_PARSER.lock().parse(scancode) else {
        return;
    };

    roxy_input::publish(event);
}

fn handle_mouse_irq() {
    let byte = I8042SecondPort::read_data();
    psaux::push_byte(byte);
}
