use alloc::{boxed::Box, sync::Arc};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{File, FileError, FileMetadata, FileType, OpenFile, SeekError, SeekFrom};
use roxy_input::InputDevice;
use roxy_terminal::{OutputError, TerminalOutput};

use crate::encoder::{EncodedInputEvent, encode_input_event};

pub(super) struct TtyFile {
    input: Arc<dyn InputDevice>,
    output: Arc<dyn TerminalOutput>,
    // Holds encoded event bytes that did not fit in the previous read buffer.
    pending: Option<EncodedInputEvent>,
    pending_offset: usize,
}

#[must_use]
pub fn open_file(input: Arc<dyn InputDevice>, output: Arc<dyn TerminalOutput>) -> Arc<OpenFile> {
    OpenFile::new(Box::new(TtyFile {
        input,
        output,
        pending: None,
        pending_offset: 0,
    }))
}

impl File for TtyFile {
    fn is_terminal(&self) -> bool {
        true
    }

    fn metadata(&self) -> Result<FileMetadata, FileError> {
        Ok(FileMetadata {
            file_id: 1,
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        })
    }

    fn read(&mut self, _position: &mut u64, output: &mut [u8]) -> Result<usize, FileError> {
        if output.is_empty() {
            return Ok(0);
        }

        loop {
            let count = self.read_available(output);

            if count > 0 {
                return Ok(count);
            }

            assert!(!CurrentArchitectureBackend::interrupts_enabled());
            CurrentArchitectureBackend::wait_for_interrupt();
        }
    }

    fn write(&mut self, _position: &mut u64, input: &[u8]) -> Result<usize, FileError> {
        self.output.write(input).map_err(map_output_error)
    }

    fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
        Err(SeekError::NotSeekable)
    }
}

impl TtyFile {
    fn read_available(&mut self, output: &mut [u8]) -> usize {
        let mut count = self.drain_pending(output);

        while count < output.len() {
            let Some(event) = self.input.read_event() else {
                break;
            };

            self.pending = encode_input_event(event);
            count += self.drain_pending(&mut output[count..]);
        }

        count
    }

    fn drain_pending(&mut self, output: &mut [u8]) -> usize {
        let Some(pending) = self.pending else {
            return 0;
        };
        let remaining = &pending.as_bytes()[self.pending_offset..];
        let count = output.len().min(remaining.len());
        output[..count].copy_from_slice(&remaining[..count]);
        self.pending_offset += count;

        if self.pending_offset == pending.as_bytes().len() {
            self.pending = None;
            self.pending_offset = 0;
        }

        count
    }
}

fn map_output_error(error: OutputError) -> FileError {
    match error {
        OutputError::Io => FileError::Io,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{sync::Arc, vec::Vec};
    use core::sync::atomic::{AtomicUsize, Ordering};

    use roxy_fd::{FileType, SeekError, SeekFrom};
    use roxy_input::{InputDevice, InputEvent, KeyCode, KeyState};
    use roxy_terminal::{OutputError, TerminalOutput};
    use roxy_test::kernel_test;
    use spin::Mutex;

    use super::open_file;

    struct MockInput;

    impl InputDevice for MockInput {
        fn read_event(&self) -> Option<InputEvent> {
            Some(InputEvent::Character('x'))
        }
    }

    struct EventInput {
        events: Mutex<Vec<InputEvent>>,
    }

    impl InputDevice for EventInput {
        fn read_event(&self) -> Option<InputEvent> {
            let mut events = self.events.lock();

            (!events.is_empty()).then(|| events.remove(0))
        }
    }

    struct MockOutput {
        written: AtomicUsize,
        bytes: Mutex<Vec<u8>>,
    }

    impl TerminalOutput for MockOutput {
        fn write(&self, input: &[u8]) -> Result<usize, OutputError> {
            self.written.fetch_add(input.len(), Ordering::Relaxed);
            self.bytes.lock().extend_from_slice(input);

            Ok(input.len())
        }
    }

    kernel_test!("roxy-ttyfd::file-adapter", delegates_tty_io, {
        let input = Arc::new(MockInput);
        let output = Arc::new(MockOutput {
            written: AtomicUsize::new(0),
            bytes: Mutex::new(Vec::new()),
        });
        let first = open_file(input.clone(), output.clone());
        let second = open_file(input, output.clone());
        let mut buffer = [0; 4];

        assert!(first.is_terminal());
        assert_eq!(
            first.metadata().unwrap().file_type,
            FileType::CharacterDevice
        );
        assert_eq!(first.metadata().unwrap().file_id, 1);
        assert_eq!(first.metadata().unwrap().permissions, 0o600);
        assert_eq!(first.read(&mut buffer), Ok(4));
        assert_eq!(&buffer, b"xxxx");
        assert_eq!(first.write(b"one"), Ok(3));
        assert_eq!(second.write(b"two"), Ok(3));
        assert_eq!(output.written.load(Ordering::Relaxed), 6);
        assert_eq!(&*output.bytes.lock(), b"onetwo");
        assert_eq!(first.seek(SeekFrom::Start(0)), Err(SeekError::NotSeekable));
        assert!(!Arc::ptr_eq(&first, &second));
    });

    kernel_test!("roxy-ttyfd::event-encoding", encodes_input_events, {
        let input = Arc::new(EventInput {
            events: Mutex::new(alloc::vec![
                InputEvent::Character('é'),
                InputEvent::Key {
                    code: KeyCode::ArrowLeft,
                    state: KeyState::Pressed,
                },
                InputEvent::Key {
                    code: KeyCode::ArrowLeft,
                    state: KeyState::Released,
                },
            ]),
        });
        let output = Arc::new(MockOutput {
            written: AtomicUsize::new(0),
            bytes: Mutex::new(Vec::new()),
        });
        let file = open_file(input, output);
        let mut first = [0; 3];
        let mut second = [0; 2];

        assert_eq!(file.read(&mut first), Ok(3));
        assert_eq!(&first, &[0xc3, 0xa9, 0x1b]);
        assert_eq!(file.read(&mut second), Ok(2));
        assert_eq!(&second, b"[D");
    });
}
