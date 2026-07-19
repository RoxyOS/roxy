#![no_std]
#![allow(clippy::missing_errors_doc)]

extern crate alloc;

use spin::Once;

mod directory;
mod error;
mod file;
mod interface;
mod io;
mod metadata;
mod mount;
mod path;
#[cfg(feature = "kernel-test")]
mod test_utils;
mod traits;

static GLOBAL_VFS: Once<Vfs> = Once::new();

pub fn register_global_vfs(vfs: Vfs) -> Result<(), VfsError> {
    if GLOBAL_VFS.get().is_some() {
        return Err(VfsError::Busy);
    }

    GLOBAL_VFS.call_once(|| vfs);

    Ok(())
}

pub(crate) fn global_vfs() -> Result<&'static Vfs, VfsError> {
    GLOBAL_VFS.get().ok_or(VfsError::NotInitialized)
}

pub use directory::DirEntry;
pub use error::VfsError;
pub use file::{CreationMode, OpenAccess, OpenOptions, SeekFrom, VfsFile};
pub use interface::{
    create, hard_link, metadata, mkdir, open, read, read_dir, read_link, rename, rmdir, symlink,
    sync, unlink, write,
};
pub use metadata::{FileType, Metadata};
pub use mount::Vfs;
pub use path::VfsPath;
pub use traits::{FileHandle, FileSystem};
