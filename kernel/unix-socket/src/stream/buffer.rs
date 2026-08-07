use alloc::collections::VecDeque;

pub(super) const CAPACITY: usize = 64 * 1024;

pub(super) struct Buffer {
    bytes: VecDeque<u8>,
}

impl Buffer {
    pub(super) const fn new() -> Self {
        Self {
            bytes: VecDeque::new(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.bytes.len()
    }

    pub(super) fn read_to(&mut self, output: &mut [u8]) -> usize {
        let count = output.len().min(self.bytes.len());

        for byte in &mut output[..count] {
            *byte = self.bytes.pop_front().unwrap();
        }

        count
    }

    pub(super) fn write_from(&mut self, input: &[u8]) -> usize {
        let count = input.len().min(CAPACITY - self.bytes.len());
        self.bytes.extend(&input[..count]);
        count
    }

    pub(super) fn clear(&mut self) {
        self.bytes.clear();
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{Buffer, CAPACITY};

    kernel_test!("roxy-unix-socket::buffer", reads_fifo_data, {
        let mut buffer = Buffer::new();
        let mut output = [0; 3];

        assert_eq!(buffer.write_from(b"abc"), 3);
        assert_eq!(buffer.read_to(&mut output), 3);
        assert_eq!(&output, b"abc");
        assert!(buffer.is_empty());
    });

    kernel_test!("roxy-unix-socket::buffer", enforces_capacity, {
        let mut buffer = Buffer::new();
        let input = alloc::vec![1; CAPACITY + 1];

        assert_eq!(buffer.write_from(&input), CAPACITY);
        assert_eq!(buffer.len(), CAPACITY);
    });
}
