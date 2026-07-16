use core::fmt::{self, Write};

use spin::{Mutex, Once};
use uart_16550::{Config, Uart16550Tty, backend::PioBackend};

type Com1 = Uart16550Tty<PioBackend>;
static SERIAL: Once<Mutex<Com1>> = Once::new();

#[macro_export]
macro_rules! s_println {
    () => {
        $crate::serial::print(format_args!("\n"))
    };
    ($($arguments:tt)*) => {
        $crate::serial::print(format_args!("{}\n", format_args!($($arguments)*)))
    };
}

#[macro_export]
macro_rules! e_println {
    () => {
        $crate::serial::emergency_print(format_args!("\n"))
    };
    ($($arguments:tt)*) => {
        $crate::serial::emergency_print(format_args!("{}\n", format_args!($($arguments)*)))
    };
}

#[allow(clippy::missing_panics_doc)]
pub(crate) fn initialize() {
    SERIAL.call_once(|| {
        // SAFETY: COM1 is exclusively owned by this module for the kernel lifetime.
        let uart = unsafe { Com1::new_port(0x3f8, Config::default()) }
            .expect("COM1 must be a valid UART port");
        Mutex::new(uart)
    });
}

pub(crate) fn print(arguments: fmt::Arguments<'_>) {
    if let Some(serial) = SERIAL.get() {
        let _ = serial.lock().write_fmt(arguments);
    }
}

pub(crate) fn emergency_print(arguments: fmt::Arguments<'_>) {
    if let Some(serial) = SERIAL.get()
        && let Some(mut serial) = serial.try_lock()
    {
        let _ = serial.write_fmt(arguments);
        return;
    }

    // SAFETY: This fallback is used only with interrupts disabled after normal
    // logging became unavailable. No concurrent COM1 access can occur locally.
    if let Ok(mut serial) = unsafe { Com1::new_port(0x3f8, Config::default()) } {
        let _ = serial.write_fmt(arguments);
    }
}
