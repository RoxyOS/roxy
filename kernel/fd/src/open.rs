use alloc::{boxed::Box, sync::Arc};

use roxy_utils::Lock;

use crate::FileError;
use crate::file::File;

pub struct OpenFile {
    object: Lock<Box<dyn File>>,
}

impl OpenFile {
    #[must_use]
    pub fn new(object: Box<dyn File>) -> Arc<Self> {
        Arc::new(Self {
            object: Lock::new(object),
        })
    }

    /// Reads through the serialized open-file state.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        self.object.lock().read(output)
    }

    /// Writes through the serialized open-file state.
    ///
    /// # Errors
    ///
    /// Returns the underlying object's error.
    pub fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        self.object.lock().write(input)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::boxed::Box;

    use roxy_test::kernel_test;

    use super::OpenFile;
    use crate::{File, FileError};

    struct Unsupported;

    impl File for Unsupported {
        fn read(&mut self, _output: &mut [u8]) -> Result<usize, FileError> {
            Err(FileError::BadOperation)
        }

        fn write(&mut self, _input: &[u8]) -> Result<usize, FileError> {
            Err(FileError::BadOperation)
        }
    }

    kernel_test!(
        "roxy-fd::open-file-ownership",
        owns_a_file_object_without_running_io,
        {
            let file = OpenFile::new(Box::new(Unsupported));
            assert_eq!(alloc::sync::Arc::strong_count(&file), 1);
        }
    );
}
