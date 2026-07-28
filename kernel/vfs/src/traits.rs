use alloc::{boxed::Box, vec::Vec};

use crate::{DirEntry, FilePermissions, Metadata, OpenOptions, ResolvedPath, SeekFrom, VfsError};

pub trait FileHandle: Send {
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, VfsError>;
    fn write(&mut self, source: &[u8]) -> Result<usize, VfsError>;
    fn seek(&mut self, position: SeekFrom) -> Result<u64, VfsError>;
    fn truncate(&mut self, size: u64) -> Result<(), VfsError>;
    fn metadata(&self) -> Result<Metadata, VfsError>;
    fn sync(&mut self) -> Result<(), VfsError>;
}

pub trait FileSystem: Send + Sync {
    fn open(
        &self,
        path: &ResolvedPath,
        options: OpenOptions,
    ) -> Result<Box<dyn FileHandle>, VfsError>;
    fn metadata(&self, path: &ResolvedPath, follow_symlink: bool) -> Result<Metadata, VfsError>;
    fn read_dir(&self, path: &ResolvedPath) -> Result<Vec<DirEntry>, VfsError>;
    fn mkdir(&self, path: &ResolvedPath, permissions: FilePermissions) -> Result<(), VfsError>;
    fn rmdir(&self, path: &ResolvedPath) -> Result<(), VfsError>;
    fn unlink(&self, path: &ResolvedPath) -> Result<(), VfsError>;
    fn hard_link(&self, source: &ResolvedPath, destination: &ResolvedPath) -> Result<(), VfsError>;
    fn symlink(&self, target: &[u8], link: &ResolvedPath) -> Result<(), VfsError>;
    fn read_link(&self, path: &ResolvedPath) -> Result<Vec<u8>, VfsError>;
    fn rename(&self, source: &ResolvedPath, destination: &ResolvedPath) -> Result<(), VfsError>;
    fn sync(&self) -> Result<(), VfsError>;
}
