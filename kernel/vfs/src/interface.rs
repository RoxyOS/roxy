use alloc::vec::Vec;

use crate::{DirEntry, Metadata, OpenOptions, VfsError, VfsFile, global_vfs};

pub fn open(path: impl AsRef<[u8]>, options: OpenOptions) -> Result<VfsFile, VfsError> {
    global_vfs()?.open(path, options)
}

pub fn create(path: impl AsRef<[u8]>) -> Result<VfsFile, VfsError> {
    global_vfs()?.create(path)
}

pub fn read(path: impl AsRef<[u8]>) -> Result<Vec<u8>, VfsError> {
    global_vfs()?.read(path)
}

pub fn write(path: impl AsRef<[u8]>, data: &[u8]) -> Result<(), VfsError> {
    global_vfs()?.write(path, data)
}

pub fn metadata(path: impl AsRef<[u8]>) -> Result<Metadata, VfsError> {
    global_vfs()?.metadata(path)
}

pub fn read_dir(path: impl AsRef<[u8]>) -> Result<Vec<DirEntry>, VfsError> {
    global_vfs()?.read_dir(path)
}

pub fn mkdir(path: impl AsRef<[u8]>) -> Result<(), VfsError> {
    global_vfs()?.mkdir(path)
}

pub fn rmdir(path: impl AsRef<[u8]>) -> Result<(), VfsError> {
    global_vfs()?.rmdir(path)
}

pub fn unlink(path: impl AsRef<[u8]>) -> Result<(), VfsError> {
    global_vfs()?.unlink(path)
}

pub fn hard_link(source: impl AsRef<[u8]>, destination: impl AsRef<[u8]>) -> Result<(), VfsError> {
    global_vfs()?.hard_link(source, destination)
}

pub fn symlink(target: impl AsRef<[u8]>, link: impl AsRef<[u8]>) -> Result<(), VfsError> {
    global_vfs()?.symlink(target, link)
}

pub fn read_link(path: impl AsRef<[u8]>) -> Result<Vec<u8>, VfsError> {
    global_vfs()?.read_link(path)
}

pub fn rename(source: impl AsRef<[u8]>, destination: impl AsRef<[u8]>) -> Result<(), VfsError> {
    global_vfs()?.rename(source, destination)
}

pub fn sync() -> Result<(), VfsError> {
    global_vfs()?.sync()
}
