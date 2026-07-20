use alloc::{boxed::Box, sync::Arc};

use roxy_fd::{File, FileError, FileMetadata, OpenFile, SeekError, SeekFrom};

use crate::TerminalDevice;

struct TerminalFile {
    device: Arc<dyn TerminalDevice>,
}

/// Creates an independent open file backed by a shared terminal endpoint.
#[must_use]
pub fn open(device: Arc<dyn TerminalDevice>) -> Arc<OpenFile> {
    OpenFile::new(Box::new(TerminalFile { device }))
}

impl File for TerminalFile {
    fn is_terminal(&self) -> bool {
        true
    }

    fn metadata(&self) -> Result<FileMetadata, FileError> {
        Ok(self.device.metadata())
    }

    fn read(&mut self, _position: &mut u64, output: &mut [u8]) -> Result<usize, FileError> {
        self.device.read(output)
    }

    fn write(&mut self, _position: &mut u64, input: &[u8]) -> Result<usize, FileError> {
        self.device.write(input)
    }

    fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
        Err(SeekError::NotSeekable)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::sync::atomic::{AtomicUsize, Ordering};

    use roxy_fd::{FileError, FileMetadata, FileType, SeekError, SeekFrom};
    use roxy_test::kernel_test;

    use super::open;
    use crate::TerminalDevice;

    struct MockTerminal {
        written: AtomicUsize,
    }

    impl TerminalDevice for MockTerminal {
        fn metadata(&self) -> FileMetadata {
            FileMetadata {
                file_id: 7,
                file_type: FileType::CharacterDevice,
                permissions: 0o600,
                size: 0,
                hard_links: 1,
            }
        }

        fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
            let read = output.len().min(1);
            output[..read].copy_from_slice(&b"x"[..read]);

            Ok(read)
        }

        fn write(&self, input: &[u8]) -> Result<usize, FileError> {
            self.written.fetch_add(input.len(), Ordering::Relaxed);

            Ok(input.len())
        }
    }

    kernel_test!("roxy-terminal::file-adapter", delegates_terminal_io, {
        let device = alloc::sync::Arc::new(MockTerminal {
            written: AtomicUsize::new(0),
        });
        let first = open(device.clone());
        let second = open(device.clone());
        let mut output = [0; 2];

        assert!(first.is_terminal());
        assert_eq!(
            first.metadata().unwrap().file_type,
            FileType::CharacterDevice
        );
        assert_eq!(first.read(&mut output), Ok(1));
        assert_eq!(output[0], b'x');
        assert_eq!(first.write(b"one"), Ok(3));
        assert_eq!(second.write(b"two"), Ok(3));
        assert_eq!(device.written.load(Ordering::Relaxed), 6);
        assert_eq!(first.seek(SeekFrom::Start(0)), Err(SeekError::NotSeekable));
    });
}
