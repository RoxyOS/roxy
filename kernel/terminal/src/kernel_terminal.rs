use alloc::sync::Arc;
use core::fmt::{self, Write};

use roxy_utils::Lock;
use spin::Once;

use crate::TerminalDevice;

static KERNEL_TERMINAL: Once<Arc<dyn TerminalDevice>> = Once::new();
static PRINT_LOCK: Lock<()> = Lock::new(());

/// Selects the terminal used for ordinary kernel output.
///
/// # Panics
///
/// Panics when a kernel terminal has already been selected.
pub fn select_kernel_terminal(terminal: Arc<dyn TerminalDevice>) {
    assert!(
        KERNEL_TERMINAL.get().is_none(),
        "kernel terminal was already selected"
    );
    KERNEL_TERMINAL.call_once(|| terminal);
}

/// Returns the selected kernel terminal.
///
/// # Panics
///
/// Panics when core has not selected a kernel terminal.
#[must_use]
pub fn kernel_terminal() -> Arc<dyn TerminalDevice> {
    KERNEL_TERMINAL
        .get()
        .expect("kernel terminal must be selected before use")
        .clone()
}

/// Writes formatted ordinary kernel output to the selected terminal.
///
/// # Panics
///
/// Panics when core has not selected a kernel terminal.
pub fn print(arguments: fmt::Arguments<'_>) {
    let _guard = PRINT_LOCK.lock();
    let terminal = KERNEL_TERMINAL
        .get()
        .expect("kernel terminal must be selected before printing");

    let _ = TerminalWriter(terminal.as_ref()).write_fmt(arguments);
}

struct TerminalWriter<'a>(&'a dyn TerminalDevice);

impl Write for TerminalWriter<'_> {
    fn write_str(&mut self, output: &str) -> fmt::Result {
        let mut written = 0;

        while written < output.len() {
            let count = self
                .0
                .write(&output.as_bytes()[written..])
                .map_err(|_| fmt::Error)?;

            if count == 0 {
                return Err(fmt::Error);
            }
            written += count;
        }

        Ok(())
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{sync::Arc, vec::Vec};
    use core::fmt::Write;

    use roxy_fd::{FileError, FileMetadata, FileType};
    use roxy_test::kernel_test;
    use spin::Mutex;

    use super::TerminalWriter;
    use crate::TerminalDevice;

    struct PartialTerminal {
        output: Mutex<Vec<u8>>,
    }

    impl TerminalDevice for PartialTerminal {
        fn metadata(&self) -> FileMetadata {
            FileMetadata {
                file_id: 9,
                file_type: FileType::CharacterDevice,
                permissions: 0o600,
                size: 0,
                hard_links: 1,
            }
        }

        fn read(&self, _output: &mut [u8]) -> Result<usize, FileError> {
            Err(FileError::BadOperation)
        }

        fn write(&self, input: &[u8]) -> Result<usize, FileError> {
            let written = input.len().min(2);
            self.output.lock().extend_from_slice(&input[..written]);

            Ok(written)
        }
    }

    kernel_test!("roxy-terminal::kernel-writer", completes_partial_writes, {
        let terminal = Arc::new(PartialTerminal {
            output: Mutex::new(Vec::new()),
        });

        write!(TerminalWriter(terminal.as_ref()), "value={}", 42).unwrap();

        assert_eq!(&*terminal.output.lock(), b"value=42");
    });
}
