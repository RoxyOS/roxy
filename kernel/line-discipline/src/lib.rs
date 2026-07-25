#![no_std]

extern crate alloc;

mod settings;

use alloc::vec::Vec;
use core::mem;

pub use settings::LineDisciplineSettings;

const INPUT_CAPACITY: usize = 4096;
const CANONICAL_PAYLOAD_CAPACITY: usize = INPUT_CAPACITY - 1;

/// Processes the byte stream between a TTY input device and its readers.
pub struct LineDiscipline {
    pub settings: LineDisciplineSettings,
    buffered: Vec<u8>,
}

impl LineDiscipline {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            settings: LineDisciplineSettings::new(),
            buffered: Vec::new(),
        }
    }

    #[must_use]
    pub const fn with_settings(settings: LineDisciplineSettings) -> Self {
        Self {
            settings,
            buffered: Vec::new(),
        }
    }

    /// Applies the current input policy to one encoded input event.
    pub fn process(&mut self, input: &[u8]) -> ProcessResult {
        if input.is_empty() {
            return ProcessResult::ignored();
        }

        if !self.settings.canonical {
            return ProcessResult {
                echo: self.settings.echo,
                buffer: Some(input.to_vec()),
            };
        }

        if input == [self.settings.erase_character] {
            let erased = self.erase_character();

            return ProcessResult {
                echo: erased && self.settings.echo,
                buffer: None,
            };
        }

        // Enter
        if input == b"\n" {
            self.buffered.push(b'\n');

            return ProcessResult {
                echo: self.settings.echo,
                buffer: Some(mem::take(&mut self.buffered)),
            };
        }

        if input.len() > CANONICAL_PAYLOAD_CAPACITY.saturating_sub(self.buffered.len()) {
            return ProcessResult::ignored();
        }

        self.buffered.extend_from_slice(input);

        ProcessResult {
            echo: self.settings.echo,
            buffer: None,
        }
    }

    /// Replaces terminal input settings.
    ///
    /// Returns unfinished canonical input when disabling canonical mode so the TTY can make it
    /// readable.
    pub fn update_settings(&mut self, settings: LineDisciplineSettings) -> Option<Vec<u8>> {
        let released =
            if self.settings.canonical && !settings.canonical && !self.buffered.is_empty() {
                Some(mem::take(&mut self.buffered))
            } else {
                None
            };

        self.settings = settings;

        released
    }

    /// Discards the line currently being edited.
    pub fn clear_input(&mut self) {
        self.buffered.clear();
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
        CANONICAL_PAYLOAD_CAPACITY, INPUT_CAPACITY, LineDiscipline, LineDisciplineSettings,
        ProcessResult,
    };

    kernel_test!("roxy-line-discipline::canonical", buffers_canonical_line, {
        let mut discipline = LineDiscipline::new();

        assert!(discipline.settings.echo);
        assert!(discipline.settings.canonical);
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
        let mut discipline = LineDiscipline::with_settings(LineDisciplineSettings {
            echo: false,
            canonical: false,
            ..LineDisciplineSettings::new()
        });

        assert_eq!(discipline.process(b"\x08").buffer.unwrap(), b"\x08");

        discipline.settings.echo = true;
        let result = discipline.process(b"x");
        assert!(result.echo);
        assert_eq!(result.buffer.unwrap(), b"x");
    });

    kernel_test!(
        "roxy-line-discipline::mode-transition",
        releases_partial_line,
        {
            let mut discipline = LineDiscipline::new();

            assert!(discipline.process(b"partial").buffer.is_none());
            let released = discipline.update_settings(LineDisciplineSettings {
                canonical: false,
                ..LineDisciplineSettings::new()
            });

            assert_eq!(released.unwrap(), b"partial");
        }
    );

    kernel_test!("roxy-line-discipline::flush", discards_partial_line, {
        let mut discipline = LineDiscipline::new();

        let _ = discipline.process(b"discarded");
        discipline.clear_input();

        assert_eq!(discipline.process(b"\n").buffer.unwrap(), b"\n");
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
