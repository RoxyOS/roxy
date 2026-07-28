use alloc::{boxed::Box, vec::Vec};
use core::sync::atomic::{AtomicUsize, Ordering};

use roxy_utils::Lock;

use crate::{
    DirEntry, FileHandle, FilePermissions, FileSystem, FileType, Metadata, OpenOptions,
    ResolvedPath, SeekFrom, VfsError,
};

pub(crate) struct MockFileSystem {
    file_id: u64,
    file_type: FileType,
    last_path: Lock<Vec<u8>>,
    syncs: AtomicUsize,
}

impl MockFileSystem {
    pub(crate) fn new(file_id: u64) -> Self {
        Self {
            file_id,
            file_type: FileType::Regular,
            last_path: Lock::new(Vec::new()),
            syncs: AtomicUsize::new(0),
        }
    }

    pub(crate) fn directory(file_id: u64) -> Self {
        Self {
            file_type: FileType::Directory,
            ..Self::new(file_id)
        }
    }

    pub(crate) fn last_path(&self) -> Vec<u8> {
        self.last_path.lock().clone()
    }

    pub(crate) fn sync_count(&self) -> usize {
        self.syncs.load(Ordering::Relaxed)
    }

    fn metadata_value(&self) -> Metadata {
        Metadata {
            file_id: self.file_id,
            file_type: self.file_type,
            permissions: crate::FilePermissions::DEFAULT_FILE,
            size: 0,
            hard_links: 1,
        }
    }
}

impl FileSystem for MockFileSystem {
    fn open(
        &self,
        path: &ResolvedPath,
        _options: OpenOptions,
    ) -> Result<Box<dyn FileHandle>, VfsError> {
        *self.last_path.lock() = path.as_bytes().into();

        Ok(Box::new(MockFile(self.metadata_value())))
    }

    fn metadata(&self, path: &ResolvedPath, _follow: bool) -> Result<Metadata, VfsError> {
        *self.last_path.lock() = path.as_bytes().into();

        Ok(self.metadata_value())
    }

    fn read_dir(&self, _path: &ResolvedPath) -> Result<Vec<DirEntry>, VfsError> {
        Ok(Vec::new())
    }

    fn mkdir(&self, _path: &ResolvedPath, _permissions: FilePermissions) -> Result<(), VfsError> {
        Ok(())
    }

    fn rmdir(&self, _path: &ResolvedPath) -> Result<(), VfsError> {
        Ok(())
    }

    fn unlink(&self, _path: &ResolvedPath) -> Result<(), VfsError> {
        Ok(())
    }

    fn hard_link(
        &self,
        _source: &ResolvedPath,
        _destination: &ResolvedPath,
    ) -> Result<(), VfsError> {
        Ok(())
    }

    fn symlink(&self, _target: &[u8], _link: &ResolvedPath) -> Result<(), VfsError> {
        Ok(())
    }

    fn read_link(&self, _path: &ResolvedPath) -> Result<Vec<u8>, VfsError> {
        Err(VfsError::Unsupported)
    }

    fn rename(&self, _source: &ResolvedPath, _destination: &ResolvedPath) -> Result<(), VfsError> {
        Ok(())
    }

    fn sync(&self) -> Result<(), VfsError> {
        self.syncs.fetch_add(1, Ordering::Relaxed);

        Ok(())
    }
}

struct MockFile(Metadata);

impl FileHandle for MockFile {
    fn read(&mut self, _destination: &mut [u8]) -> Result<usize, VfsError> {
        Ok(0)
    }

    fn write(&mut self, source: &[u8]) -> Result<usize, VfsError> {
        Ok(source.len())
    }

    fn seek(&mut self, _position: SeekFrom) -> Result<u64, VfsError> {
        Ok(0)
    }

    fn truncate(&mut self, size: u64) -> Result<(), VfsError> {
        self.0.size = size;

        Ok(())
    }

    fn metadata(&self) -> Result<Metadata, VfsError> {
        Ok(self.0)
    }

    fn sync(&mut self) -> Result<(), VfsError> {
        Ok(())
    }
}
