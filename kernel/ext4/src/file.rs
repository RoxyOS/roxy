use alloc::sync::Arc;

use ext4plus::file::File;
use roxy_block::BlockDevice;
use roxy_utils::Lock;
use roxy_vfs::{FileHandle, Metadata, OpenOptions, SeekFrom, VfsError};

use crate::{error::map_ext4, metadata};

pub(crate) struct Ext4File {
    pub(crate) file: File,
    pub(crate) options: OpenOptions,
    pub(crate) mutation: Arc<Lock<()>>,
    pub(crate) device: &'static dyn BlockDevice,
}

impl FileHandle for Ext4File {
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, VfsError> {
        if !self.options.can_read() {
            return Err(VfsError::PermissionDenied);
        }

        self.file.read_bytes(destination).map_err(map_ext4)
    }

    fn write(&mut self, source: &[u8]) -> Result<usize, VfsError> {
        if !self.options.can_write() {
            return Err(VfsError::PermissionDenied);
        }

        let _mutation = self.mutation.lock();

        if self.options.append {
            self.file
                .seek_to(self.file.inode().size_in_bytes())
                .map_err(map_ext4)?;
        }

        self.file.write_bytes(source).map_err(map_ext4)
    }

    fn seek(&mut self, position: SeekFrom) -> Result<u64, VfsError> {
        let position = match position {
            SeekFrom::Start(position) => position,
            SeekFrom::Current(offset) => add_offset(self.file.position(), offset)?,
            SeekFrom::End(offset) => add_offset(self.file.inode().size_in_bytes(), offset)?,
        };
        self.file.seek_to(position).map_err(map_ext4)?;

        Ok(position)
    }

    fn truncate(&mut self, size: u64) -> Result<(), VfsError> {
        if !self.options.can_write() {
            return Err(VfsError::PermissionDenied);
        }

        let _mutation = self.mutation.lock();

        self.file.truncate(size).map_err(map_ext4)
    }

    fn metadata(&self) -> Result<Metadata, VfsError> {
        Ok(metadata::from_inode(self.file.inode()))
    }

    fn sync(&mut self) -> Result<(), VfsError> {
        self.device.flush().map_err(|_| VfsError::Io)
    }
}

fn add_offset(base: u64, offset: i64) -> Result<u64, VfsError> {
    if offset >= 0 {
        base.checked_add(offset.unsigned_abs())
            .ok_or(VfsError::InvalidInput)
    } else {
        base.checked_sub(offset.unsigned_abs())
            .ok_or(VfsError::InvalidInput)
    }
}
