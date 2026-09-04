use alloc::{boxed::Box, sync::Arc, vec::Vec};

use roxy_fd::{IoctlError, IoctlRequest, MmapError, MmapTarget, PollEvents};

use crate::{DirEntry, FilePermissions, Metadata, OpenOptions, ResolvedPath, SeekFrom, VfsError};

pub trait FileHandle: Send {
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, VfsError>;
    fn write(&mut self, source: &[u8]) -> Result<usize, VfsError>;
    fn seek(&mut self, position: SeekFrom) -> Result<u64, VfsError>;
    fn truncate(&mut self, size: u64) -> Result<(), VfsError>;
    fn metadata(&self) -> Result<Metadata, VfsError>;
    fn sync(&mut self) -> Result<(), VfsError>;

    /// Reports whether the object behind this handle is a terminal.
    ///
    /// The default is `false` for regular files; character-device handles that expose a terminal
    /// (e.g. a pty slave) override it so `isatty` on the descriptor reports `true`.
    fn is_terminal(&self) -> bool {
        false
    }

    /// Returns this terminal's openable device pathname, when the handle is a terminal whose
    /// controlling device is reachable through the device filesystem (for example `/dev/tty0` or
    /// `/dev/pts/0`).
    ///
    /// A `ttyname` consumer reopens the returned path, so it must resolve to a registered device
    /// node. Non-terminal handles return `None`.
    fn terminal_path(&self) -> Option<Vec<u8>> {
        None
    }

    /// Sets the permission bits of the file behind this handle.
    ///
    /// # Errors
    ///
    /// Returns an error when the object does not support permission changes.
    fn set_permissions(&mut self, permissions: FilePermissions) -> Result<(), VfsError>;

    /// Performs a typed ioctl operation.
    ///
    /// # Errors
    ///
    /// Returns an operation-specific ioctl error.
    fn ioctl(&mut self, _request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        Err(IoctlError::NotTty)
    }

    /// Describes the physical memory backing a file-backed `mmap`.
    ///
    /// # Errors
    ///
    /// Returns an error when the object does not support device mapping.
    fn mmap(&mut self, _size: usize, _offset: u64) -> Result<MmapTarget, MmapError> {
        Err(MmapError::Unsupported)
    }

    /// Reports the events that are currently ready for this handle.
    ///
    /// The default implementation reports the handle as both readable and writable, which is
    /// correct for regular files and most filesystem-backed handles. Character-device handles
    /// should override this to reflect the actual device readiness.
    fn poll(&self) -> Result<PollEvents, VfsError> {
        Ok(PollEvents {
            readable: true,
            writable: true,
            ..PollEvents::default()
        })
    }

    /// Registers a listener to be notified when this handle's readiness may have changed.
    ///
    /// The default implementation returns an inactive registration (no-op). Handles that can
    /// become ready asynchronously (e.g. device files) should override this.
    fn register_poll_listener(
        &self,
        _listener: Arc<roxy_poll::PollListener>,
    ) -> roxy_poll::PollRegistration {
        roxy_poll::PollRegistration::inactive()
    }
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

    /// Sets the permission bits of the file or directory at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error when the path cannot be resolved or the change cannot be persisted.
    fn set_permissions(
        &self,
        path: &ResolvedPath,
        permissions: FilePermissions,
    ) -> Result<(), VfsError>;
    fn unlink(&self, path: &ResolvedPath) -> Result<(), VfsError>;
    fn hard_link(&self, source: &ResolvedPath, destination: &ResolvedPath) -> Result<(), VfsError>;
    fn symlink(&self, target: &[u8], link: &ResolvedPath) -> Result<(), VfsError>;
    fn read_link(&self, path: &ResolvedPath) -> Result<Vec<u8>, VfsError>;
    fn rename(&self, source: &ResolvedPath, destination: &ResolvedPath) -> Result<(), VfsError>;
    fn sync(&self) -> Result<(), VfsError>;
}
