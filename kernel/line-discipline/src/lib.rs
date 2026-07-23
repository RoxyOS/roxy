#![no_std]

/// Processes the byte stream between a TTY input device and its readers.
pub struct LineDiscipline {
    pub termios: Termios,
}

impl LineDiscipline {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            termios: Termios::new(),
        }
    }

    #[must_use]
    pub const fn with_termios(termios: Termios) -> Self {
        Self { termios }
    }

    /// Applies the current input policy to one byte.
    #[must_use]
    pub fn process(&mut self, byte: u8) -> ProcessResult {
        ProcessResult {
            input: Some(byte),
            echo: self.termios.echo.then_some(byte),
        }
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
}

impl Termios {
    #[must_use]
    pub const fn new() -> Self {
        Self { echo: true }
    }
}

impl Default for Termios {
    fn default() -> Self {
        Self::new()
    }
}

/// Describes the TTY actions produced by processing one input byte.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessResult {
    pub input: Option<u8>,
    pub echo: Option<u8>,
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{LineDiscipline, ProcessResult, Termios};

    kernel_test!("roxy-line-discipline::echo", passes_through_and_echoes, {
        let mut discipline = LineDiscipline::new();

        for byte in [b'x', 0xc3, 0xa9, b'\n', 0x1b] {
            assert_eq!(
                discipline.process(byte),
                ProcessResult {
                    input: Some(byte),
                    echo: Some(byte),
                }
            );
        }
    });

    kernel_test!("roxy-line-discipline::echo-setting", obeys_echo_setting, {
        let mut discipline = LineDiscipline::with_termios(Termios { echo: false });

        assert_eq!(
            discipline.process(b'x'),
            ProcessResult {
                input: Some(b'x'),
                echo: None,
            }
        );

        discipline.termios.echo = true;
        assert_eq!(discipline.process(b'y').echo, Some(b'y'));
    });
}
