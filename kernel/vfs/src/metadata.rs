use crate::{FilePermissions, Vfs, VfsError, VfsPath};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileType {
    Regular,
    Directory,
    Symlink,
    BlockDevice,
    CharacterDevice,
    Fifo,
    Socket,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Metadata {
    pub file_id: u64,
    pub file_type: FileType,
    pub permissions: FilePermissions,
    pub size: u64,
    pub hard_links: u32,
}

impl Vfs {
    pub fn metadata(&self, path: impl AsRef<[u8]>) -> Result<Metadata, VfsError> {
        let path = VfsPath::new(path)?;
        let resolved = self.resolve(&path)?;

        resolved.filesystem.metadata(&resolved.local_path, true)
    }
}
