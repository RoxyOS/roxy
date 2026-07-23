use alloc::{boxed::Box, sync::Arc};

use roxy_fd::{File, FileError, FileMetadata, FileType, OpenFile, SeekError, SeekFrom};

use crate::Tty;

pub(super) struct TtyFile {
    tty: Arc<Tty>,
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
        self.tty.read(output)
    }

    fn write(&mut self, _position: &mut u64, input: &[u8]) -> Result<usize, FileError> {
        self.tty.write(input)
    }

    fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
        Err(SeekError::NotSeekable)
    }
}

impl TtyFile {
    #[must_use]
    pub(super) fn open(tty: Arc<Tty>) -> Arc<OpenFile> {
        OpenFile::new(Box::new(Self { tty }))
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{sync::Arc, vec::Vec};
    use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use roxy_fd::{FileError, FileType, SeekError, SeekFrom};
    use roxy_input::{InputDevice, InputEvent, KeyCode, KeyState};
    use roxy_terminal::{OutputError, TerminalOutput};
    use roxy_test::kernel_test;
    use spin::Mutex;

    use super::TtyFile;
    use crate::Tty;

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
        fail_next: AtomicBool,
        zero_next: AtomicBool,
    }

    impl MockOutput {
        fn new() -> Self {
            Self {
                written: AtomicUsize::new(0),
                bytes: Mutex::new(Vec::new()),
                fail_next: AtomicBool::new(false),
                zero_next: AtomicBool::new(false),
            }
        }
    }

    impl TerminalOutput for MockOutput {
        fn write(&self, input: &[u8]) -> Result<usize, OutputError> {
            if self.fail_next.swap(false, Ordering::Relaxed) {
                return Err(OutputError::Io);
            }

            if self.zero_next.swap(false, Ordering::Relaxed) {
                return Ok(0);
            }

            self.written.fetch_add(input.len(), Ordering::Relaxed);
            self.bytes.lock().extend_from_slice(input);

            Ok(input.len())
        }
    }

    fn open_file(
        input: Arc<dyn InputDevice>,
        output: Arc<dyn TerminalOutput>,
    ) -> Arc<roxy_fd::OpenFile> {
        TtyFile::open(Arc::new(Tty::new(input, output)))
    }

    kernel_test!("roxy-ttyfd::file-adapter", delegates_tty_io, {
        let input = Arc::new(MockInput);
        let output = Arc::new(MockOutput::new());
        let tty = Arc::new(Tty::new(input, output.clone()));
        let first = TtyFile::open(tty.clone());
        let second = TtyFile::open(tty);
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
        assert_eq!(output.written.load(Ordering::Relaxed), 10);
        assert_eq!(&*output.bytes.lock(), b"xxxxonetwo");
        assert_eq!(first.seek(SeekFrom::Start(0)), Err(SeekError::NotSeekable));
        assert!(!Arc::ptr_eq(&first, &second));
    });

    kernel_test!("roxy-ttyfd::event-encoding", encodes_and_echoes_events, {
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
        let output = Arc::new(MockOutput::new());
        let file = open_file(input, output.clone());
        let mut first = [0; 3];
        let mut second = [0; 2];

        assert_eq!(file.read(&mut first), Ok(3));
        assert_eq!(&first, &[0xc3, 0xa9, 0x1b]);
        assert_eq!(file.read(&mut second), Ok(2));
        assert_eq!(&second, b"[D");
        assert_eq!(&*output.bytes.lock(), &[0xc3, 0xa9, 0x1b, b'[', b'D']);
    });

    kernel_test!("roxy-ttyfd::shared-input-state", shares_tty_input_state, {
        let input = Arc::new(EventInput {
            events: Mutex::new(alloc::vec![InputEvent::Character('é')]),
        });
        let output = Arc::new(MockOutput::new());
        let tty = Arc::new(Tty::new(input, output.clone()));
        let first = TtyFile::open(tty.clone());
        let second = TtyFile::open(tty.clone());
        let mut first_byte = [0; 1];
        let mut second_byte = [0; 1];

        assert_eq!(Arc::strong_count(&tty), 3);
        assert!(!Arc::ptr_eq(&first, &second));
        assert_eq!(first.read(&mut first_byte), Ok(1));
        assert_eq!(second.read(&mut second_byte), Ok(1));
        assert_eq!(first_byte, [0xc3]);
        assert_eq!(second_byte, [0xa9]);
        assert_eq!(&*output.bytes.lock(), &[0xc3, 0xa9]);
    });

    kernel_test!("roxy-ttyfd::disabled-echo", skips_disabled_echo, {
        let input = Arc::new(EventInput {
            events: Mutex::new(alloc::vec![InputEvent::Character('x')]),
        });
        let output = Arc::new(MockOutput::new());
        let tty = Arc::new(Tty::new(input, output.clone()));
        tty.line_discipline.lock().termios.echo = false;
        let file = TtyFile::open(tty);
        let mut buffer = [0; 1];

        assert_eq!(file.read(&mut buffer), Ok(1));
        assert_eq!(&buffer, b"x");
        assert!(output.bytes.lock().is_empty());
    });

    kernel_test!("roxy-ttyfd::echo-retry", retries_failed_echo, {
        let input = Arc::new(EventInput {
            events: Mutex::new(alloc::vec![
                InputEvent::Character('x'),
                InputEvent::Character('y'),
            ]),
        });
        let output = Arc::new(MockOutput::new());
        let file = open_file(input, output.clone());
        let mut buffer = [0; 1];

        output.fail_next.store(true, Ordering::Relaxed);
        assert_eq!(file.read(&mut buffer), Err(FileError::Io));
        assert_eq!(file.read(&mut buffer), Ok(1));
        assert_eq!(&buffer, b"x");
        assert_eq!(&*output.bytes.lock(), b"x");

        output.zero_next.store(true, Ordering::Relaxed);
        assert_eq!(file.read(&mut buffer), Err(FileError::Io));
        assert_eq!(file.read(&mut buffer), Ok(1));
        assert_eq!(&buffer, b"y");
        assert_eq!(&*output.bytes.lock(), b"xy");
    });
}
