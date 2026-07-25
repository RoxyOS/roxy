use alloc::sync::Arc;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::FileError;
use roxy_input::InputDevice;
use roxy_line_discipline::{LineDiscipline, ProcessResult};
use roxy_terminal::{OutputError, TerminalOutput};
use roxy_utils::Lock;

use crate::Tty;
use crate::encoder::{EncodedInputEvent, encode_input_event};

impl Tty {
    pub(super) fn new(input: Arc<dyn InputDevice>, output: Arc<dyn TerminalOutput>) -> Self {
        let window_size = output.window_size();

        Self {
            input,
            output,
            line_discipline: Lock::new(LineDiscipline::new()),
            window_size: Lock::new(window_size),
            buffered: Lock::new(alloc::vec::Vec::new()),
            read_lock: Lock::new(()),
        }
    }

    pub(super) fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        if output.is_empty() {
            return Ok(0);
        }

        loop {
            let count = self.read_inner(output)?;

            if count > 0 {
                return Ok(count);
            }

            assert!(!CurrentArchitectureBackend::interrupts_enabled());
            CurrentArchitectureBackend::wait_for_interrupt();
        }
    }

    pub(super) fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        self.output.write(input).map_err(map_output_error)
    }

    fn read_inner(&self, output: &mut [u8]) -> Result<usize, FileError> {
        let _read_guard = self.read_lock.lock();

        loop {
            let count = self.drain_buffered(output);

            if count == 0 {
                let Some(event) = self.next_input_event() else {
                    return Ok(0);
                };

                let result = self.line_discipline.lock().process(event.as_bytes());
                self.apply_result(event.as_bytes(), result)?;
            } else {
                return Ok(count);
            }
        }
    }

    fn apply_result(&self, input: &[u8], result: ProcessResult) -> Result<(), FileError> {
        if let Some(buffer) = result.buffer {
            self.buffered.lock().extend(buffer);
        }

        if result.echo {
            let written = self.output.write(input).map_err(map_output_error)?;

            if written != input.len() {
                return Err(FileError::Io);
            }
        }

        Ok(())
    }

    fn drain_buffered(&self, output: &mut [u8]) -> usize {
        let mut buffered = self.buffered.lock();
        let count = output.len().min(buffered.len());

        output[..count].copy_from_slice(&buffered[..count]);

        let remaining = buffered.len() - count;

        buffered.copy_within(count.., 0);
        buffered.truncate(remaining);

        count
    }

    /// Wait for the next input event
    fn next_input_event(&self) -> Option<EncodedInputEvent> {
        loop {
            let event = self.input.read_event()?;

            if let Some(encoded) = encode_input_event(event) {
                return Some(encoded);
            }
        }
    }
}

fn map_output_error(error: OutputError) -> FileError {
    match error {
        OutputError::Io => FileError::Io,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::FileError;
    use roxy_input::{InputEvent, KeyCode, KeyState};
    use roxy_test::kernel_test;

    use crate::test_support::{character, open};

    kernel_test!("roxy-tty::noncanonical", preserves_event_stream, {
        let events = alloc::vec![
            character('é'),
            InputEvent::Key {
                code: KeyCode::ArrowLeft,
                state: KeyState::Pressed,
            },
            InputEvent::Key {
                code: KeyCode::ArrowLeft,
                state: KeyState::Released,
            },
        ];
        let (tty, output, file) = open(events);
        tty.line_discipline.lock().settings.canonical = false;
        let mut first = [0; 2];
        let mut second = [0; 3];

        assert_eq!(file.read(&mut first), Ok(2));
        assert_eq!(&first, &[0xc3, 0xa9]);
        assert_eq!(file.read(&mut second), Ok(3));
        assert_eq!(&second, b"\x1b[D");
        assert_eq!(output.bytes(), &[0xc3, 0xa9, 0x1b, b'[', b'D']);
    });

    kernel_test!("roxy-tty::partial-echo", preserves_buffered_input, {
        let events = alloc::vec![InputEvent::Key {
            code: KeyCode::ArrowLeft,
            state: KeyState::Pressed,
        }];
        let (tty, output, file) = open(events);
        tty.line_discipline.lock().settings.canonical = false;
        output.limit_writes(1);
        let mut buffer = [0; 3];

        assert_eq!(file.read(&mut buffer), Err(FileError::Io));
        assert_eq!(output.bytes(), b"\x1b");
        assert_eq!(file.read(&mut buffer), Ok(3));
        assert_eq!(&buffer, b"\x1b[D");
    });

    kernel_test!("roxy-tty::echo-error", preserves_committed_line, {
        let (_tty, output, file) = open(alloc::vec![character('x'), character('\n')]);
        output.fail_on_call(2);
        let mut buffer = [0; 2];

        assert_eq!(file.read(&mut buffer), Err(FileError::Io));
        assert_eq!(output.bytes(), b"x");
        assert_eq!(file.read(&mut buffer), Ok(2));
        assert_eq!(&buffer, b"x\n");
    });
}
