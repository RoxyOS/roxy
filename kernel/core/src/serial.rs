use alloc::sync::Arc;
use core::fmt::{self, Write};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{Fd, FdTable, FileError, FileMetadata, FileType};
use roxy_terminal::TerminalDevice;
use roxy_utils::Lock;
use spin::Once;
use uart_16550::{Config, Uart16550Tty, backend::PioBackend};

type Com1 = Uart16550Tty<PioBackend>;
static SERIAL: Once<Lock<Com1>> = Once::new();

struct SerialTerminal;

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
        Lock::new(uart)
    });
    roxy_utils::unsupported::initialize(print);
}

pub(crate) fn print(arguments: fmt::Arguments<'_>) {
    let serial = SERIAL.get().expect("serial must be initialized");
    let _ = serial.lock().write_fmt(arguments);
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

pub(crate) fn inject_initial_fds(table: &mut FdTable) {
    let terminal: Arc<dyn TerminalDevice> = Arc::new(SerialTerminal);

    for expected in [Fd::new(0), Fd::new(1), Fd::new(2)] {
        let inserted = table.insert(roxy_terminal::open(terminal.clone()));

        assert_eq!(inserted, expected, "initial FD table was not empty");
    }
}

impl TerminalDevice for SerialTerminal {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: 1,
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        }
    }

    fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        if output.is_empty() {
            return Ok(0);
        }

        loop {
            let read = SERIAL
                .get()
                .expect("serial must be initialized")
                .lock()
                .inner_mut()
                .receive_bytes(output);

            if read > 0 {
                return Ok(read);
            }

            assert!(CurrentArchitectureBackend::interrupts_enabled());
            CurrentArchitectureBackend::halt();
        }
    }

    fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        let mut serial = SERIAL.get().expect("serial must be initialized").lock();

        for &byte in input {
            let bytes = if byte == b'\n' {
                b"\r\n".as_slice()
            } else {
                core::slice::from_ref(&byte)
            };

            serial.inner_mut().send_bytes_exact(bytes);
        }

        Ok(input.len())
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::sync::Arc;

    use roxy_fd::{Fd, FdTable};

    use super::inject_initial_fds;

    roxy_test::kernel_test!("roxy-kernel::initial-standard-fds", initial_standard_fds, {
        let mut table = FdTable::new();

        inject_initial_fds(&mut table);

        let stdin = table.get(Fd::new(0)).unwrap();
        let stdout = table.get(Fd::new(1)).unwrap();
        let stderr = table.get(Fd::new(2)).unwrap();
        assert!(stdin.is_terminal());
        assert!(stdout.is_terminal());
        assert!(stderr.is_terminal());
        assert!(!Arc::ptr_eq(&stdin, &stdout));
        assert!(!Arc::ptr_eq(&stdout, &stderr));
        assert!(table.get(Fd::new(3)).is_none());
    });
}
