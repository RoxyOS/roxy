use crate::{FilePermissions, ResolvedPath, Vfs, VfsError};

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
    pub fn metadata(&self, path: &ResolvedPath) -> Result<Metadata, VfsError> {
        self.metadata_inner(path, true)
    }

    pub fn symlink_metadata(&self, path: &ResolvedPath) -> Result<Metadata, VfsError> {
        self.metadata_inner(path, false)
    }

    fn metadata_inner(
        &self,
        path: &ResolvedPath,
        follow_symlink: bool,
    ) -> Result<Metadata, VfsError> {
        let resolved = self.resolve(path)?;

        resolved
            .filesystem
            .metadata(&resolved.local_path, follow_symlink)
    }
}
