use alloc::sync::Arc;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::FileError;
use roxy_input::InputDevice;
use roxy_line_discipline::{LineDiscipline, ProcessResult};
use roxy_terminal::{OutputError, TerminalOutput};
use roxy_utils::Lock;

use crate::Tty;
use crate::encoder::encode_input_event;

impl Tty {
    pub(super) fn new(input: Arc<dyn InputDevice>, output: Arc<dyn TerminalOutput>) -> Self {
        Self {
            input,
            output,
            line_discipline: Lock::new(LineDiscipline::new()),
            pending: Lock::new(None),
            pending_offset: Lock::new(0),
            pending_result: Lock::new(None),
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
        let mut count = 0;

        while count < output.len() {
            let Some(result) = self.next_input_result() else {
                break;
            };

            match self.apply_result(result, &mut output[count..]) {
                Ok(written) => {
                    *self.pending_result.lock() = None;
                    count += written;
                }
                Err(error) if count == 0 => return Err(error),
                Err(_) => break,
            }
        }

        Ok(count)
    }

    /// Applies `result` to `output`.
    fn apply_result(&self, result: ProcessResult, output: &mut [u8]) -> Result<usize, FileError> {
        if let Some(byte) = result.echo {
            let written = self.output.write(&[byte]).map_err(map_output_error)?;

            if written != 1 {
                return Err(FileError::Io);
            }
        }

        let Some(byte) = result.input else {
            return Ok(0);
        };

        output[0] = byte;

        Ok(1)
    }

    fn next_input_result(&self) -> Option<ProcessResult> {
        if let Some(result) = *self.pending_result.lock() {
            return Some(result);
        }

        let byte = self.next_input_byte()?;
        let result = self.line_discipline.lock().process(byte);
        *self.pending_result.lock() = Some(result);

        Some(result)
    }

    fn next_input_byte(&self) -> Option<u8> {
        loop {
            let pending = {
                let mut pending = self.pending.lock();

                // Read from input if no pending.
                if pending.is_none() {
                    let event = self.input.read_event()?;
                    *pending = encode_input_event(event);
                }

                *pending
            };

            let Some(pending) = pending else {
                continue;
            };

            let mut pending_offset = self.pending_offset.lock();
            let byte = pending.as_bytes()[*pending_offset];
            *pending_offset += 1;

            if *pending_offset == pending.as_bytes().len() {
                *self.pending.lock() = None;
                *pending_offset = 0;
            }

            return Some(byte);
        }
    }
}

fn map_output_error(error: OutputError) -> FileError {
    match error {
        OutputError::Io => FileError::Io,
    }
}
