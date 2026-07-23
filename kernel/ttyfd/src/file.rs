use alloc::{boxed::Box, sync::Arc};

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_fd::{File, FileError, FileMetadata, FileType, OpenFile, SeekError, SeekFrom};
use roxy_input::InputDevice;
use roxy_terminal::{OutputError, TerminalOutput};

pub(super) struct TtyFile {
    input: Arc<dyn InputDevice>,
    output: Arc<dyn TerminalOutput>,
}

#[must_use]
pub fn open_file(input: Arc<dyn InputDevice>, output: Arc<dyn TerminalOutput>) -> Arc<OpenFile> {
    OpenFile::new(Box::new(TtyFile { input, output }))
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
    fn read_available(&self, output: &mut [u8]) -> usize {
        let mut count = 0;

        while count < output.len() {
            let Some(byte) = self.input.read_byte() else {
                break;
            };

            output[count] = byte;
            count += 1;
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
    use roxy_input::InputDevice;
    use roxy_terminal::{OutputError, TerminalOutput};
    use roxy_test::kernel_test;
    use spin::Mutex;

    use super::open_file;

    struct MockInput;

    impl InputDevice for MockInput {
        fn read_byte(&self) -> Option<u8> {
            Some(b'x')
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
}
