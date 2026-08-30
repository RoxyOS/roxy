use crate::{OpenFile, TruncateError};

impl OpenFile {
    /// Changes the underlying object's length without changing the open-file position.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's truncate error.
    pub fn truncate(&self, size: u64) -> Result<(), TruncateError> {
        self.state.lock().object.truncate(size)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::boxed::Box;

    use roxy_test::kernel_test;

    use super::OpenFile;
    use crate::{File, FileError, FileMetadata, PollEvents, SeekError, SeekFrom, TruncateError};

    struct Truncatable {
        length: u64,
    }

    impl File for Truncatable {
        fn poll(&mut self) -> Result<PollEvents, FileError> {
            Ok(PollEvents::default())
        }

        fn is_terminal(&self) -> bool {
            false
        }

        fn metadata(&self) -> Result<FileMetadata, FileError> {
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

        fn truncate(&mut self, size: u64) -> Result<(), TruncateError> {
            self.length = size;
            Ok(())
        }

        fn seek(&mut self, current: u64, position: SeekFrom) -> Result<u64, SeekError> {
            match position {
                SeekFrom::Start(position) => Ok(position),
                SeekFrom::Current(0) => Ok(current),
                _ => Err(SeekError::InvalidOffset),
            }
        }
    }

    kernel_test!("roxy-fd::truncate", truncates_without_moving_position, {
        let file = OpenFile::new(Box::new(Truncatable { length: 10 }));

        assert_eq!(file.seek(SeekFrom::Start(7)), Ok(7));
        assert_eq!(file.truncate(3), Ok(()));
        assert_eq!(file.seek(SeekFrom::Current(0)), Ok(7));
    });
}
