use alloc::{boxed::Box, sync::Arc, vec::Vec};

use roxy_poll::{PollListener, PollRegistration};
use roxy_utils::Lock;

use crate::file::File;
use crate::{
    DirectoryEntry, FileError, FileMetadata, PollEvents, SeekError, SeekFrom, SocketOps,
    StatusFlags,
};

pub(crate) struct OpenFileState {
    pub(crate) object: Box<dyn File>,
    position: u64,
    status_flags: StatusFlags,
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
                status_flags: StatusFlags::default(),
            }),
        })
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        self.state.lock().object.is_terminal()
    }

    /// Returns this terminal's openable device pathname, if the underlying object is a terminal
    /// backed by a device-filesystem node (e.g. `/dev/tty0` or `/dev/pts/0`).
    #[must_use]
    pub fn terminal_path(&self) -> Option<alloc::vec::Vec<u8>> {
        self.state.lock().object.terminal_path()
    }

    /// Returns metadata for the underlying open object.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn metadata(&self) -> Result<FileMetadata, FileError> {
        self.state.lock().object.metadata()
    }

    /// Reports the file status flags of this open file description.
    #[must_use]
    pub fn status_flags(&self) -> StatusFlags {
        self.state.lock().status_flags
    }

    /// Sets the file status flags of this open file description.
    pub fn set_status_flags(&self, flags: StatusFlags) {
        self.state.lock().status_flags = flags;
    }

    /// Reads through the serialized open-file state.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        let mut state = self.state.lock();
        let OpenFileState {
            object,
            position,
            status_flags,
        } = &mut *state;
        let nonblocking = status_flags.contains(StatusFlags::NONBLOCK);

        object.read(position, output, nonblocking)
    }

    /// Reads through the serialized open-file state, with optional per-call nonblocking.
    ///
    /// When `nonblocking` is `true`, the read skips waiting even if `O_NONBLOCK` is not set on
    /// the file description. Passing `false` defers to the file description's `O_NONBLOCK` flag.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn read_with_nonblocking(
        &self,
        output: &mut [u8],
        nonblocking: bool,
    ) -> Result<usize, FileError> {
        let mut state = self.state.lock();
        let OpenFileState {
            object,
            position,
            status_flags,
        } = &mut *state;
        let nonblocking = nonblocking || status_flags.contains(StatusFlags::NONBLOCK);

        object.read(position, output, nonblocking)
    }

    /// Writes through the serialized open-file state, with optional per-call nonblocking.
    ///
    /// When `nonblocking` is `true`, the write skips waiting even if `O_NONBLOCK` is not set on
    /// the file description. Passing `false` defers to the file description's `O_NONBLOCK` flag.
    ///
    /// When the open file description has `O_APPEND` set, the write position is forced to the
    /// end of the file before writing, matching append semantics regardless of any prior seek.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn write_with_nonblocking(
        &self,
        input: &[u8],
        nonblocking: bool,
    ) -> Result<usize, FileError> {
        let mut state = self.state.lock();
        let OpenFileState {
            object,
            position,
            status_flags,
        } = &mut *state;

        if status_flags.contains(StatusFlags::APPEND) {
            *position = object.seek(0, SeekFrom::End(0)).map_err(map_seek_error)?;
        }

        let nonblocking = nonblocking || status_flags.contains(StatusFlags::NONBLOCK);

        object.write(position, input, nonblocking)
    }

    /// Writes through the serialized open-file state.
    ///
    /// When the open file description has `O_APPEND` set, the write position is forced to the
    /// end of the file before writing, matching append semantics regardless of any prior seek.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        let mut state = self.state.lock();
        let OpenFileState {
            object,
            position,
            status_flags,
        } = &mut *state;

        if status_flags.contains(StatusFlags::APPEND) {
            *position = object.seek(0, SeekFrom::End(0)).map_err(map_seek_error)?;
        }

        object.write(
            position,
            input,
            status_flags.contains(StatusFlags::NONBLOCK),
        )
    }

    /// Flushes the serialized open-file object.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn sync(&self) -> Result<(), FileError> {
        self.state.lock().object.sync()
    }

    /// Sets the permission bits of the serialized open-file object.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn set_permissions(&self, permissions: u16) -> Result<(), FileError> {
        self.state.lock().object.set_permissions(permissions)
    }

    /// Reports readiness through the serialized open-file object.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's readiness error.
    pub fn poll(&self) -> Result<PollEvents, FileError> {
        self.state.lock().object.poll()
    }

    /// Registers a listener with the serialized open-file object.
    #[must_use]
    pub fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.state.lock().object.register_poll_listener(listener)
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
        let OpenFileState {
            object, position, ..
        } = &mut *state;
        let directory = object.as_directory().ok_or(FileError::BadOperation)?;

        directory.read_entries(position, limit)
    }

    /// Runs a socket operation through the serialized open-file state.
    ///
    /// Returns `None` when the open object does not support socket operations. Blocking socket
    /// operations hold the open-file lock while waiting, matching blocking reads and writes.
    pub fn socket_ops<R>(&self, operation: impl FnOnce(&mut dyn SocketOps) -> R) -> Option<R> {
        let mut state = self.state.lock();
        let socket = state.object.as_socket()?;

        Some(operation(socket))
    }
}

fn map_seek_error(_: SeekError) -> FileError {
    FileError::Io
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

        fn read(
            &mut self,
            _position: &mut u64,
            _output: &mut [u8],
            _nonblocking: bool,
        ) -> Result<usize, FileError> {
            Err(FileError::BadOperation)
        }

        fn write(
            &mut self,
            _position: &mut u64,
            _input: &[u8],
            _nonblocking: bool,
        ) -> Result<usize, FileError> {
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

        fn read(
            &mut self,
            position: &mut u64,
            output: &mut [u8],
            _nonblocking: bool,
        ) -> Result<usize, FileError> {
            let available = self.length.saturating_sub(*position);
            let available = usize::try_from(available).unwrap_or(usize::MAX);
            let read = output.len().min(available);
            *position += u64::try_from(read).unwrap();

            Ok(read)
        }

        fn write(
            &mut self,
            position: &mut u64,
            input: &[u8],
            _nonblocking: bool,
        ) -> Result<usize, FileError> {
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

    kernel_test!("roxy-fd::status-flags", read_default_and_set, {
        let file = OpenFile::new(Box::new(Cursor { length: 0 }));

        assert_eq!(file.status_flags(), crate::StatusFlags::default());
        assert_eq!(file.status_flags().bits(), 0);

        let flags = crate::StatusFlags::READ_WRITE | crate::StatusFlags::APPEND;
        file.set_status_flags(flags);
        assert_eq!(file.status_flags(), flags);
    });

    kernel_test!("roxy-fd::append-write", appends_at_end_of_file, {
        let file = OpenFile::new(Box::new(Cursor { length: 4 }));
        file.set_status_flags(crate::StatusFlags::APPEND);

        // Seek away from the end, then write; APPEND must force the position back to the end.
        assert_eq!(file.seek(SeekFrom::Start(1)), Ok(1));
        assert_eq!(file.write(b"ab"), Ok(2));

        assert_eq!(file.seek(SeekFrom::Current(0)), Ok(6));
        assert_eq!(file.seek(SeekFrom::Start(2)), Ok(2));
        assert_eq!(file.write(b"c"), Ok(1));
        assert_eq!(file.seek(SeekFrom::Current(0)), Ok(7));
    });
}
