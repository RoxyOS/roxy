use core::fmt::{self, Write};

use roxy_utils::Lock;
use spin::Once;
use uart_16550::{Config, Uart16550Tty, backend::PioBackend};

type Com1 = Uart16550Tty<PioBackend>;
static SERIAL: Once<SerialDevice> = Once::new();

pub(super) struct SerialDevice {
    uart: Lock<Com1>,
}

impl SerialDevice {
    fn new() -> Self {
        // SAFETY: COM1 is exclusively owned by this subsystem for the kernel lifetime.
        let uart = unsafe { Com1::new_port(0x3f8, Config::default()) }
            .expect("COM1 must be a valid UART port");

        Self {
            uart: Lock::new(uart),
        }
    }

    pub(super) fn send<'a>(&self, chunks: impl IntoIterator<Item = &'a [u8]>) {
        let mut uart = self.uart.lock();

        for chunk in chunks {
            uart.inner_mut().send_bytes_exact(chunk);
        }
    }

    pub(super) fn write_formatted(&self, arguments: fmt::Arguments<'_>) {
        let _ = self.uart.lock().write_fmt(arguments);
    }

    fn try_write_formatted(&self, arguments: fmt::Arguments<'_>) -> bool {
        let Some(mut uart) = self.uart.try_lock() else {
            return false;
        };

        let _ = uart.write_fmt(arguments);

        true
    }
}

pub(super) fn initialize() {
    SERIAL.call_once(SerialDevice::new);
}

pub(super) fn current() -> &'static SerialDevice {
    SERIAL.get().expect("serial must be initialized")
}

pub(super) fn emergency_write(arguments: fmt::Arguments<'_>) {
    if let Some(serial) = SERIAL.get()
        && serial.try_write_formatted(arguments)
    {
        return;
    }

    // SAFETY: This fallback is used only with interrupts disabled after normal
    // logging became unavailable. No concurrent COM1 access can occur locally.

    if let Ok(mut serial) = unsafe { Com1::new_port(0x3f8, Config::default()) } {
        let _ = serial.write_fmt(arguments);
    }
}
