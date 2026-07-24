use alloc::vec::Vec;

use crate::{
    DirEntry, Metadata, OpenOptions, ResolvedPath, VfsDirectory, VfsError, VfsFile, global_vfs,
};

pub fn open(path: impl AsRef<[u8]>, options: OpenOptions) -> Result<VfsFile, VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.open(&path, options)
}

pub fn create(path: impl AsRef<[u8]>) -> Result<VfsFile, VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.create(&path)
}

pub fn read(path: impl AsRef<[u8]>) -> Result<Vec<u8>, VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.read(&path)
}

pub fn write(path: impl AsRef<[u8]>, data: &[u8]) -> Result<(), VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.write(&path, data)
}

pub fn metadata(path: impl AsRef<[u8]>) -> Result<Metadata, VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.metadata(&path)
}

pub fn symlink_metadata(path: impl AsRef<[u8]>) -> Result<Metadata, VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.symlink_metadata(&path)
}

pub fn read_dir(path: impl AsRef<[u8]>) -> Result<Vec<DirEntry>, VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.read_dir(&path)
}

pub fn open_dir(path: impl AsRef<[u8]>) -> Result<VfsDirectory, VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.open_dir(&path)
}

pub fn mkdir(path: impl AsRef<[u8]>) -> Result<(), VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.mkdir(&path)
}

pub fn rmdir(path: impl AsRef<[u8]>) -> Result<(), VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.rmdir(&path)
}

pub fn unlink(path: impl AsRef<[u8]>) -> Result<(), VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.unlink(&path)
}

pub fn hard_link(source: impl AsRef<[u8]>, destination: impl AsRef<[u8]>) -> Result<(), VfsError> {
    let vfs = global_vfs()?;
    let source = ResolvedPath::resolve(source)?;
    let destination = ResolvedPath::resolve(destination)?;

    vfs.hard_link(&source, &destination)
}

pub fn symlink(target: impl AsRef<[u8]>, link: impl AsRef<[u8]>) -> Result<(), VfsError> {
    let vfs = global_vfs()?;
    let link = ResolvedPath::resolve(link)?;

    vfs.symlink(target, &link)
}

pub fn read_link(path: impl AsRef<[u8]>) -> Result<Vec<u8>, VfsError> {
    let vfs = global_vfs()?;
    let path = ResolvedPath::resolve(path)?;

    vfs.read_link(&path)
}

pub fn rename(source: impl AsRef<[u8]>, destination: impl AsRef<[u8]>) -> Result<(), VfsError> {
    let vfs = global_vfs()?;
    let source = ResolvedPath::resolve(source)?;
    let destination = ResolvedPath::resolve(destination)?;

    vfs.rename(&source, &destination)
}

pub fn sync() -> Result<(), VfsError> {
    global_vfs()?.sync()
}
