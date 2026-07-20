use alloc::sync::Arc;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{FileError, FileMetadata, FileType};
use roxy_terminal::TerminalDevice;
use spin::Once;

use crate::device;

struct SerialTerminal;
static TERMINAL: Once<Arc<SerialTerminal>> = Once::new();

/// Returns a terminal endpoint backed by the initialized serial device.
#[must_use]
pub fn terminal() -> Arc<dyn TerminalDevice> {
    TERMINAL.call_once(|| Arc::new(SerialTerminal)).clone()
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
            let read = device::current().receive(output);

            if read > 0 {
                return Ok(read);
            }

            assert!(CurrentArchitectureBackend::interrupts_enabled());
            CurrentArchitectureBackend::halt();
        }
    }

    fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        let chunks = input.iter().map(|byte| {
            if *byte == b'\n' {
                b"\r\n".as_slice()
            } else {
                core::slice::from_ref(byte)
            }
        });

        device::current().send(chunks);

        Ok(input.len())
    }
}
