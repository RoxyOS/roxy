use alloc::sync::Arc;

use roxy_terminal::{OutputError, TerminalOutput};
use spin::Once;

use crate::device;

struct SerialTerminal;
static TERMINAL: Once<Arc<SerialTerminal>> = Once::new();

/// Returns a terminal endpoint backed by the initialized serial device.
#[must_use]
pub fn terminal() -> Arc<dyn TerminalOutput> {
    TERMINAL.call_once(|| Arc::new(SerialTerminal)).clone()
}

impl TerminalOutput for SerialTerminal {
    fn write(&self, input: &[u8]) -> Result<usize, OutputError> {
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
