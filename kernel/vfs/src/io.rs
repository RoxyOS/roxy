use alloc::vec::Vec;

use crate::{CreationMode, OpenAccess, OpenOptions, Vfs, VfsError};

impl Vfs {
    pub fn read(&self, path: impl AsRef<[u8]>) -> Result<Vec<u8>, VfsError> {
        let mut file = self.open(path, OpenOptions::read_only())?;
        let mut output = Vec::new();
        let mut buffer = [0_u8; 4096];

        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..read]);
        }

        Ok(output)
    }

    pub fn write(&self, path: impl AsRef<[u8]>, data: &[u8]) -> Result<(), VfsError> {
        let options = OpenOptions {
            access: OpenAccess::WriteOnly,
            creation: CreationMode::Create,
            permissions: crate::FilePermissions::DEFAULT_FILE,
            append: false,
            truncate: true,
        };
        let mut file = self.open(path, options)?;
        let mut written = 0;

        while written < data.len() {
            let count = file.write(&data[written..])?;
            if count == 0 {
                return Err(VfsError::Io);
            }
            written += count;
        }

        Ok(())
    }
}
