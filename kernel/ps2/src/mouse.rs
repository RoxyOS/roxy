use heapless::Deque;

/// Maximum number of raw mouse bytes queued before the oldest are dropped.
const MOUSE_CAPACITY: usize = 256;

/// Owns the bounded queue of raw PS/2 mouse bytes delivered by IRQ12.
///
/// The bytes are forwarded verbatim to `/dev/psaux`; protocol parsing (button state, X/Y deltas,
/// wheel) is left to the consuming driver (xf86-input-mouse), matching how Linux exposes the
/// auxiliary PS/2 device.
pub(crate) struct MouseInput {
    bytes: Deque<u8, MOUSE_CAPACITY>,
}

impl MouseInput {
    pub(crate) const fn new() -> Self {
        Self {
            bytes: Deque::new(),
        }
    }

    /// Queues one raw byte, dropping the oldest when the queue is full.
    pub(crate) fn push(&mut self, byte: u8) {
        if self.bytes.is_full() {
            let _ = self.bytes.pop_front();
        }
        let _ = self.bytes.push_back(byte);
    }

    /// Copies up to `output.len()` queued bytes into `output`, returning how many were read.
    pub(crate) fn read_into(&mut self, output: &mut [u8]) -> usize {
        let mut read = 0;

        while read < output.len() {
            let Some(byte) = self.bytes.pop_front() else {
                break;
            };
            output[read] = byte;
            read += 1;
        }

        read
    }

    /// Reports whether no bytes are currently queued.
    pub(crate) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{MOUSE_CAPACITY, MouseInput};

    kernel_test!("roxy-ps2::mouse-queue", queues_and_drains_bytes, {
        let mut input = MouseInput::new();
        assert!(input.is_empty());
        assert_eq!(input.read_into(&mut [0; 4]), 0);

        for value in 0..4u8 {
            input.push(value);
        }

        assert!(!input.is_empty());
        let mut output = [0u8; 2];
        assert_eq!(input.read_into(&mut output), 2);
        assert_eq!(output, [0, 1]);
        assert_eq!(input.read_into(&mut output), 2);
        assert_eq!(output, [2, 3]);
        assert!(input.is_empty());
    });

    kernel_test!("roxy-ps2::mouse-queue-full", drops_oldest_when_full, {
        let mut input = MouseInput::new();
        for value in 0..MOUSE_CAPACITY {
            input.push(value.try_into().expect("value fits in u8"));
        }
        // One more byte evicts the oldest (0) and appends at the tail.
        input.push(0xff);

        let mut output = [0u8; 2];
        assert_eq!(input.read_into(&mut output), 2);
        assert_eq!(output, [1, 2]);
    });
}
