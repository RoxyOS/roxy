use alloc::{boxed::Box, collections::BTreeMap, sync::Arc};

use roxy_utils::Lock;

use crate::{FileHandle, FilePermissions, Metadata, Vfs, VfsError, VfsPath};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OpenAccess {
    #[default]
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CreationMode {
    #[default]
    OpenExisting,
    Create,
    CreateNew,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenOptions {
    pub access: OpenAccess,
    pub creation: CreationMode,
    pub permissions: FilePermissions,
    pub append: bool,
    pub truncate: bool,
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self::read_only()
    }
}

impl OpenOptions {
    #[must_use]
    pub const fn read_only() -> Self {
        Self {
            access: OpenAccess::ReadOnly,
            creation: CreationMode::OpenExisting,
            permissions: FilePermissions::DEFAULT_FILE,
            append: false,
            truncate: false,
        }
    }

    #[must_use]
    pub const fn create() -> Self {
        Self {
            access: OpenAccess::WriteOnly,
            creation: CreationMode::Create,
            permissions: FilePermissions::DEFAULT_FILE,
            append: false,
            truncate: false,
        }
    }

    #[must_use]
    pub const fn can_read(self) -> bool {
        matches!(self.access, OpenAccess::ReadOnly | OpenAccess::ReadWrite)
    }

    #[must_use]
    pub const fn can_write(self) -> bool {
        matches!(self.access, OpenAccess::WriteOnly | OpenAccess::ReadWrite)
    }

    pub fn validate(self) -> Result<(), VfsError> {
        if (self.creation != CreationMode::OpenExisting || self.truncate || self.append)
            && !self.can_write()
        {
            return Err(VfsError::InvalidInput);
        }

        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeekFrom {
    Start(u64),
    Current(i64),
    End(i64),
}

/// Tracks open VFS handles by inode ID for one mounted filesystem.
///
/// The counts let namespace operations reject mutations of an active inode and let unmounting
/// reject a mount that still has open files.
pub(crate) struct ActiveHandles {
    files: Lock<BTreeMap<u64, usize>>,
}

impl Default for ActiveHandles {
    fn default() -> Self {
        Self {
            files: Lock::new(BTreeMap::new()),
        }
    }
}

impl ActiveHandles {
    /// Records one newly opened handle for an inode.
    pub(crate) fn add(&self, file_id: u64) {
        let mut files = self.files.lock();
        *files.entry(file_id).or_default() += 1;
    }

    /// Removes one handle and drops the inode entry when its count reaches zero.
    pub(crate) fn remove(&self, file_id: u64) {
        let mut files = self.files.lock();
        let count = files
            .get_mut(&file_id)
            .expect("active VFS handle is tracked");

        *count -= 1;

        if *count == 0 {
            files.remove(&file_id);
        }
    }

    /// Returns whether an inode currently has an open handle.
    pub(crate) fn contains(&self, file_id: u64) -> bool {
        self.files.lock().contains_key(&file_id)
    }

    /// Returns whether any file on the mounted filesystem remains open.
    pub(crate) fn any(&self) -> bool {
        !self.files.lock().is_empty()
    }
}

pub struct VfsFile {
    pub(crate) handle: Box<dyn FileHandle>,
    pub(crate) file_id: u64,
    pub(crate) active: Arc<ActiveHandles>,
}

impl VfsFile {
    pub fn read(&mut self, destination: &mut [u8]) -> Result<usize, VfsError> {
        self.handle.read(destination)
    }

    pub fn write(&mut self, source: &[u8]) -> Result<usize, VfsError> {
        self.handle.write(source)
    }

    pub fn seek(&mut self, position: SeekFrom) -> Result<u64, VfsError> {
        self.handle.seek(position)
    }

    pub fn truncate(&mut self, size: u64) -> Result<(), VfsError> {
        self.handle.truncate(size)
    }

    pub fn metadata(&self) -> Result<Metadata, VfsError> {
        self.handle.metadata()
    }

    pub fn sync(&mut self) -> Result<(), VfsError> {
        self.handle.sync()
    }
}

impl Vfs {
    pub fn open(&self, path: impl AsRef<[u8]>, options: OpenOptions) -> Result<VfsFile, VfsError> {
        options.validate()?;
        let path = VfsPath::new(path)?;
        let resolved = self.resolve(&path)?;
        let handle = resolved.filesystem.open(&resolved.local_path, options)?;
        let file_id = handle.metadata()?.file_id;

        resolved.active.add(file_id);

        Ok(VfsFile {
            handle,
            file_id,
            active: resolved.active,
        })
    }

    pub fn create(&self, path: impl AsRef<[u8]>) -> Result<VfsFile, VfsError> {
        self.open(path, OpenOptions::create())
    }
}

impl Drop for VfsFile {
    fn drop(&mut self) {
        self.active.remove(self.file_id);
    }
}
