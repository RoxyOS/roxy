#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::mem;

const INPUT_CAPACITY: usize = 4096;
const CANONICAL_PAYLOAD_CAPACITY: usize = INPUT_CAPACITY - 1;

/// Processes the byte stream between a TTY input device and its readers.
pub struct LineDiscipline {
    pub termios: Termios,
    buffered: Vec<u8>,
}

impl LineDiscipline {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            termios: Termios::new(),
            buffered: Vec::new(),
        }
    }

    #[must_use]
    pub const fn with_termios(termios: Termios) -> Self {
        Self {
            termios,
            buffered: Vec::new(),
        }
    }

    /// Applies the current input policy to one encoded input event.
    #[must_use]
    pub fn process(&mut self, input: &[u8]) -> ProcessResult {
        if input.is_empty() {
            return ProcessResult::ignored();
        }

        assert!(
            self.termios.canonical,
            "non-canonical is not supported for now"
        );

        // Backspace
        if input == b"\x08" {
            let erased = self.erase_character();

            return ProcessResult {
                echo: erased && self.termios.echo,
                buffer: None,
            };
        }

        // Enter
        if input == b"\n" {
            self.buffered.push(b'\n');

            return ProcessResult {
                echo: self.termios.echo,
                buffer: Some(mem::take(&mut self.buffered)),
            };
        }

        if input.len() > CANONICAL_PAYLOAD_CAPACITY.saturating_sub(self.buffered.len()) {
            return ProcessResult::ignored();
        }

        self.buffered.extend_from_slice(input);

        ProcessResult {
            echo: self.termios.echo,
            buffer: None,
        }
    }

    /// Erase the last buffered character. Returns weather it actually
    /// erased a character. If the buffer is empty it returns false.
    fn erase_character(&mut self) -> bool {
        let Some(start) = self.buffered.iter().rposition(|byte| byte & 0xc0 != 0x80) else {
            return false;
        };

        self.buffered.truncate(start);

        true
    }
}

impl Default for LineDiscipline {
    fn default() -> Self {
        Self::new()
    }
}

/// Terminal input settings owned by a line discipline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Termios {
    pub echo: bool,
    pub canonical: bool,
}

impl Termios {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            echo: true,
            canonical: true,
        }
    }
}

impl Default for Termios {
    fn default() -> Self {
        Self::new()
    }
}

/// Describes the TTY actions produced by processing one encoded input event.
#[must_use]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessResult {
    pub echo: bool,
    /// Bytes that become readable at the TTY layer.
    pub buffer: Option<Vec<u8>>,
}

impl ProcessResult {
    const fn ignored() -> Self {
        Self {
            echo: false,
            buffer: None,
        }
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{
        CANONICAL_PAYLOAD_CAPACITY, INPUT_CAPACITY, LineDiscipline, ProcessResult, Termios,
    };

    kernel_test!("roxy-line-discipline::canonical", buffers_canonical_line, {
        let mut discipline = LineDiscipline::new();

        assert!(discipline.termios.echo);
        assert!(discipline.termios.canonical);
        assert_eq!(
            discipline.process(b"ab"),
            ProcessResult {
                echo: true,
                buffer: None,
            }
        );
        assert_eq!(discipline.process(b"\n").buffer.unwrap(), b"ab\n");
    });

    kernel_test!("roxy-line-discipline::erase", erases_utf8_character, {
        let mut discipline = LineDiscipline::new();

        assert!(discipline.process("aé".as_bytes()).echo);
        assert!(discipline.process(b"\x08").echo);

        assert_eq!(discipline.process(b"\n").buffer.unwrap(), b"a\n");
    });

    kernel_test!("roxy-line-discipline::empty-erase", ignores_empty_erase, {
        let mut discipline = LineDiscipline::new();

        assert_eq!(discipline.process(b"\x08"), ProcessResult::ignored());
    });

    kernel_test!("roxy-line-discipline::settings", obeys_settings, {
        let mut discipline = LineDiscipline::with_termios(Termios {
            echo: false,
            canonical: false,
        });

        assert_eq!(discipline.process(b"\x08").buffer.unwrap(), b"\x08");

        discipline.termios.echo = true;
        let result = discipline.process(b"x");
        assert!(result.echo);
        assert_eq!(result.buffer.unwrap(), b"x");
    });

    kernel_test!("roxy-line-discipline::capacity", bounds_canonical_input, {
        let mut discipline = LineDiscipline::new();

        for _ in 0..CANONICAL_PAYLOAD_CAPACITY {
            assert!(discipline.process(b"x").echo);
        }

        assert_eq!(discipline.process("é".as_bytes()), ProcessResult::ignored());
        let buffer = discipline.process(b"\n").buffer.unwrap();

        assert_eq!(buffer.len(), INPUT_CAPACITY);
        assert!(
            buffer[..CANONICAL_PAYLOAD_CAPACITY]
                .iter()
                .all(|byte| *byte == b'x')
        );
        assert_eq!(buffer[CANONICAL_PAYLOAD_CAPACITY], b'\n');
    });
}
