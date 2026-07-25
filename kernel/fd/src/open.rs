use alloc::{boxed::Box, sync::Arc, vec::Vec};

use roxy_utils::Lock;

use crate::file::File;
use crate::{DirectoryEntry, FileError, FileMetadata, PollEvents, SeekError, SeekFrom};

pub(crate) struct OpenFileState {
    pub(crate) object: Box<dyn File>,
    position: u64,
}

pub struct OpenFile {
    pub(crate) state: Lock<OpenFileState>,
}

impl OpenFile {
    #[must_use]
    pub fn new(object: Box<dyn File>) -> Arc<Self> {
        Arc::new(Self {
            state: Lock::new(OpenFileState {
                object,
                position: 0,
            }),
        })
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.state.lock().object.is_terminal()
    }

    /// Returns metadata for the underlying open object.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn metadata(&self) -> Result<FileMetadata, FileError> {
        self.state.lock().object.metadata()
    }

    /// Reads through the serialized open-file state.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        let mut state = self.state.lock();
        let OpenFileState { object, position } = &mut *state;

        object.read(position, output)
    }

    /// Writes through the serialized open-file state.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        let mut state = self.state.lock();
        let OpenFileState { object, position } = &mut *state;

        object.write(position, input)
    }

    /// Reports readiness through the serialized open-file object.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's readiness error.
    pub fn poll(&self) -> Result<PollEvents, FileError> {
        self.state.lock().object.poll()
    }

    /// Changes and returns the serialized open-file position.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's seek error.
    pub fn seek(&self, position: SeekFrom) -> Result<u64, SeekError> {
        let mut state = self.state.lock();
        let current = state.position;
        let new_position = state.object.seek(current, position)?;

        if new_position > i64::MAX.cast_unsigned() {
            return Err(SeekError::Overflow);
        }

        state.position = new_position;

        Ok(new_position)
    }

    /// Reads directory entries through the serialized open-file state if `self` is a directory.
    ///
    /// # Errors
    ///
    /// Returns `BadOperation` when the open object is not a directory.
    pub fn read_directory_entries(&self, limit: usize) -> Result<Vec<DirectoryEntry>, FileError> {
        let mut state = self.state.lock();
        let OpenFileState { object, position } = &mut *state;
        let directory = object.as_directory().ok_or(FileError::BadOperation)?;

        directory.read_entries(position, limit)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::boxed::Box;

    use roxy_test::kernel_test;

    use super::OpenFile;
    use crate::{File, FileError, PollEvents, SeekError, SeekFrom};

    struct Unsupported {
        terminal: bool,
    }

    struct Cursor {
        length: u64,
    }

    impl File for Unsupported {
        fn poll(&mut self) -> Result<PollEvents, FileError> {
            Ok(PollEvents::default())
        }

        fn is_terminal(&self) -> bool {
            self.terminal
        }

        fn metadata(&self) -> Result<crate::FileMetadata, FileError> {
            Err(FileError::BadOperation)
        }

        fn read(&mut self, _position: &mut u64, _output: &mut [u8]) -> Result<usize, FileError> {
            Err(FileError::BadOperation)
        }

        fn write(&mut self, _position: &mut u64, _input: &[u8]) -> Result<usize, FileError> {
            Err(FileError::BadOperation)
        }

        fn seek(&mut self, _current: u64, _position: SeekFrom) -> Result<u64, SeekError> {
            Err(SeekError::NotSeekable)
        }
    }

    impl File for Cursor {
        fn poll(&mut self) -> Result<PollEvents, FileError> {
            Ok(PollEvents {
                readable: true,
                writable: true,
                ..PollEvents::default()
            })
        }

        fn is_terminal(&self) -> bool {
            false
        }

        fn metadata(&self) -> Result<crate::FileMetadata, FileError> {
            Ok(crate::FileMetadata {
                file_id: 1,
                file_type: crate::FileType::Regular,
                permissions: 0o644,
                size: self.length,
                hard_links: 1,
            })
        }

        fn read(&mut self, position: &mut u64, output: &mut [u8]) -> Result<usize, FileError> {
            let available = self.length.saturating_sub(*position);
            let available = usize::try_from(available).unwrap_or(usize::MAX);
            let read = output.len().min(available);
            *position += u64::try_from(read).unwrap();

            Ok(read)
        }

        fn write(&mut self, position: &mut u64, input: &[u8]) -> Result<usize, FileError> {
            let written = u64::try_from(input.len()).map_err(|_| FileError::Io)?;
            *position = position.checked_add(written).ok_or(FileError::Io)?;
            self.length = self.length.max(*position);

            Ok(input.len())
        }

        fn seek(&mut self, current: u64, position: SeekFrom) -> Result<u64, SeekError> {
            let position = match position {
                SeekFrom::Start(position) => position,
                SeekFrom::Current(offset) => relative(current, offset)?,
                SeekFrom::End(offset) => relative(self.length, offset)?,
            };

            Ok(position)
        }
    }

    fn relative(base: u64, offset: i64) -> Result<u64, SeekError> {
        base.checked_add_signed(offset).ok_or(if offset < 0 {
            SeekError::InvalidOffset
        } else {
            SeekError::Overflow
        })
    }

    kernel_test!(
        "roxy-fd::open-file-ownership",
        owns_a_file_object_without_running_io,
        {
            let file = OpenFile::new(Box::new(Unsupported { terminal: false }));
            assert_eq!(alloc::sync::Arc::strong_count(&file), 1);
            assert_eq!(file.seek(SeekFrom::Start(0)), Err(SeekError::NotSeekable));
        }
    );

    kernel_test!("roxy-fd::terminal-property", reports_terminal_property, {
        let terminal = OpenFile::new(Box::new(Unsupported { terminal: true }));
        let non_terminal = OpenFile::new(Box::new(Unsupported { terminal: false }));

        assert!(terminal.is_terminal());
        assert!(!non_terminal.is_terminal());
    });

    kernel_test!("roxy-fd::default-poll", reports_default_readiness, {
        let file = OpenFile::new(Box::new(Cursor { length: 0 }));
        let ready = file.poll().unwrap();

        assert_eq!(
            ready,
            PollEvents {
                readable: true,
                writable: true,
                ..PollEvents::default()
            }
        );
    });

    kernel_test!("roxy-fd::shared-seek-position", shared_seek_position, {
        let file = OpenFile::new(Box::new(Cursor { length: 10 }));
        let shared = file.clone();
        let independent = OpenFile::new(Box::new(Cursor { length: 10 }));

        assert_eq!(file.seek(SeekFrom::Start(4)), Ok(4));
        assert_eq!(shared.seek(SeekFrom::Current(3)), Ok(7));
        assert_eq!(file.seek(SeekFrom::End(-2)), Ok(8));
        assert_eq!(
            shared.seek(SeekFrom::Current(-9)),
            Err(SeekError::InvalidOffset)
        );
        assert_eq!(file.seek(SeekFrom::Current(0)), Ok(8));
        assert_eq!(
            file.seek(SeekFrom::Start(u64::MAX)),
            Err(SeekError::Overflow)
        );
        assert_eq!(shared.seek(SeekFrom::Current(0)), Ok(8));
        assert_eq!(independent.write(&[1, 2, 3]), Ok(3));
        assert_eq!(independent.seek(SeekFrom::Current(0)), Ok(3));
        assert_eq!(file.seek(SeekFrom::Current(0)), Ok(8));

        let mut output = [0; 4];
        assert_eq!(shared.read(&mut output), Ok(2));
        assert_eq!(file.seek(SeekFrom::Current(0)), Ok(10));
        assert_eq!(independent.seek(SeekFrom::Current(0)), Ok(3));
    });
}
