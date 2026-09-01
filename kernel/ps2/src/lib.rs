#![no_std]

extern crate alloc;

#[cfg(not(target_arch = "x86_64"))]
compile_error!("roxy-ps2 currently supports only x86_64 i8042 controllers");

mod i8042;
mod input;
mod mouse;
mod psaux;
mod scancode;

use core::sync::atomic::{AtomicBool, Ordering};

use alloc::sync::Arc;

use roxy_arch::{Architecture, CurrentArchitectureBackend, IrqLine};
use roxy_input::{InputDevice, InputListener, InputListeners, KeyEvent};
use roxy_utils::Lock;
use spin::Once;

use i8042::{I8042FirstPort, I8042SecondPort};
use input::KeyboardInput;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static KEYBOARD_INPUT: Lock<KeyboardInput> = Lock::new(KeyboardInput::new());
static INPUT_LISTENERS: InputListeners = InputListeners::new();
static INPUT_DEVICE: Once<Arc<Ps2InputDevice>> = Once::new();

struct Ps2InputDevice;

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
    INPUT_DEVICE.call_once(|| Arc::new(Ps2InputDevice));
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
    let result = KEYBOARD_INPUT.lock().process_scancode(scancode);

    if result.is_ok() {
        INPUT_LISTENERS.notify();
    }
}

fn handle_mouse_irq() {
    let byte = I8042SecondPort::read_data();
    psaux::push_byte(byte);
}

impl InputDevice for Ps2InputDevice {
    fn read_key(&self) -> Option<KeyEvent> {
        CurrentArchitectureBackend::without_interrupts(|| KEYBOARD_INPUT.lock().read())
    }

    fn register_listener(&self, listener: Arc<dyn InputListener>) {
        INPUT_LISTENERS.register(&listener);
    }
}

#[cfg(feature = "kernel-test")]
pub fn inject_for_test(events: &[KeyEvent]) {
    CurrentArchitectureBackend::without_interrupts(|| {
        let mut input = KEYBOARD_INPUT.lock();

        for &event in events {
            let _ = input.enqueue_event(event);
        }
    });

    INPUT_LISTENERS.notify();
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_input::{KeyCode, KeyEvent, KeyState};
    use roxy_test::kernel_test;

    fn event(code: KeyCode, state: KeyState) -> KeyEvent {
        KeyEvent { code, state }
    }

    kernel_test!("roxy-ps2::input-device", reads_injected_key_events, {
        let events = [
            event(KeyCode::A, KeyState::Pressed),
            event(KeyCode::B, KeyState::Pressed),
        ];
        super::inject_for_test(&events);

        assert_eq!(
            super::input_device().read_key(),
            Some(event(KeyCode::A, KeyState::Pressed))
        );
        assert_eq!(
            super::input_device().read_key(),
            Some(event(KeyCode::B, KeyState::Pressed))
        );
        assert_eq!(super::input_device().read_key(), None);
    });
}
