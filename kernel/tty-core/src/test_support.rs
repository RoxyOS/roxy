//! Test scaffolding shared by `roxy-tty-core`'s kernel-tests: a byte-queue input source and a
//! mock output endpoint.

use alloc::sync::Arc;

use roxy_tty_types::WindowSize;
use spin::Mutex;

use crate::core::TtyCore;
use crate::input::TerminalInputSource;
use crate::output::{OutputError, TtyOutput};

pub(crate) struct ByteQueue {
    queue: Mutex<alloc::vec::Vec<u8>>,
}

impl ByteQueue {
    pub(crate) fn push(&self, bytes: &[u8]) {
        self.queue.lock().extend_from_slice(bytes);
    }

    // A pty/master input source feeds its stream to the line discipline one byte at a time, so a
    // newline arrives as its own event and canonical mode can commit it. Mirror that here.
    fn pop_byte(&self) -> Option<u8> {
        let mut queue = self.queue.lock();
        if queue.is_empty() {
            return None;
        }
        let byte = queue.remove(0);
        Some(byte)
    }
}

impl TerminalInputSource for ByteQueue {
    fn next_input_bytes(&self) -> Option<alloc::vec::Vec<u8>> {
        Some(alloc::vec![self.pop_byte()?])
    }

    fn try_peek_bytes(&self) -> Option<alloc::vec::Vec<u8>> {
        Some(alloc::vec![*self.queue.lock().first()?])
    }

    fn consume_peeked(&self) {
        let _ = self.pop_byte();
    }

    fn discard_pending_input(&self) {
        self.queue.lock().clear();
    }
}

pub(crate) struct MockOutput {
    bytes: Mutex<alloc::vec::Vec<u8>>,
    window_size: WindowSize,
}

impl MockOutput {
    fn new() -> Self {
        Self {
            bytes: Mutex::new(alloc::vec::Vec::new()),
            window_size: WindowSize {
                rows: 30,
                columns: 100,
                pixel_width: 800,
                pixel_height: 480,
            },
        }
    }

    pub(crate) fn bytes(&self) -> alloc::vec::Vec<u8> {
        self.bytes.lock().clone()
    }
}

impl TtyOutput for MockOutput {
    fn write(&self, input: &[u8]) -> Result<usize, OutputError> {
        self.bytes.lock().extend_from_slice(input);
        Ok(input.len())
    }

    fn window_size(&self) -> WindowSize {
        self.window_size
    }
}

pub(crate) fn open() -> (Arc<TtyCore>, Arc<ByteQueue>, Arc<MockOutput>) {
    let output = Arc::new(MockOutput::new());
    let source = Arc::new(ByteQueue {
        queue: Mutex::new(alloc::vec::Vec::new()),
    });
    let core = TtyCore::new(output.clone(), source.clone());

    (core, source, output)
}
