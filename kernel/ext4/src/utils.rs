use ext4plus::{
    FollowSymlinks,
    dir::Dir,
    inode::{Inode, InodeCreationOptions, InodeFlags, InodeMode},
    path::Path,
};
use roxy_vfs::{FilePermissions, FileType, VfsError, ResolvedPath};

use crate::{Ext4FileSystem, error::map_ext4};

impl Ext4FileSystem {
    /// Resolves a VFS path to its ext4 inode.
    ///
    /// `follow_final_symlink` controls whether the final path component is followed when it is a
    /// symbolic link. Symbolic links in intermediate components are always followed.
    pub(crate) fn resolve_inode(
        &self,
        path: &ResolvedPath,
        follow_final_symlink: bool,
    ) -> Result<Inode, VfsError> {
        let path = Path::try_from(path.as_bytes()).map_err(|_| VfsError::InvalidPath)?;
        let follow = if follow_final_symlink {
            FollowSymlinks::All
        } else {
            FollowSymlinks::ExcludeFinalComponent
        };

        self.filesystem
            .path_to_inode(path, follow)
            .map_err(map_ext4)
    }

    pub(crate) fn parent<'a>(
        &self,
        path: &'a ResolvedPath,
    ) -> Result<(Dir, ext4plus::DirEntryName<'a>), VfsError> {
        let bytes = path.as_bytes();
        let separator = bytes
            .iter()
            .rposition(|byte| *byte == b'/')
            .ok_or(VfsError::InvalidPath)?;
        let name = ext4plus::DirEntryName::try_from(&bytes[separator + 1..])
            .map_err(|_| VfsError::InvalidPath)?;
        let parent_bytes = if separator == 0 {
            b"/".as_slice()
        } else {
            &bytes[..separator]
        };
        let parent = ResolvedPath::resolve(parent_bytes)?;
        let inode = self.resolve_inode(&parent, true)?;

        Ok((
            Dir::open_inode(&self.filesystem, inode).map_err(map_ext4)?,
            name,
        ))
    }

    pub(crate) fn new_inode(
        &self,
        file_type: FileType,
        permissions: FilePermissions,
    ) -> Result<Inode, VfsError> {
        let (ext_type, mode) = match file_type {
            FileType::Regular => (ext4plus::FileType::Regular, InodeMode::S_IFREG),
            FileType::Directory => (ext4plus::FileType::Directory, InodeMode::S_IFDIR),
            _ => return Err(VfsError::Unsupported),
        };
        let mode = mode | InodeMode::from_bits_retain(permissions.bits());

        self.filesystem
            .create_inode(InodeCreationOptions {
                file_type: ext_type,
                mode,
                uid: 0,
                gid: 0,
                time: roxy_time::realtime_time(),
                flags: InodeFlags::empty(),
            })
            .map_err(map_ext4)
    }

    pub(crate) fn create_regular(
        &self,
        path: &ResolvedPath,
        permissions: FilePermissions,
    ) -> Result<(), VfsError> {
        let (mut parent, name) = self.parent(path)?;
        let mut inode = self.new_inode(FileType::Regular, permissions)?;

        parent.link(name, &mut inode).map_err(map_ext4)
    }

    pub(crate) fn empty_directory(&self, path: &ResolvedPath) -> Result<bool, VfsError> {
        for entry in self
            .filesystem
            .read_dir(path.as_bytes())
            .map_err(map_ext4)?
        {
            let entry = entry.map_err(map_ext4)?;
            if entry.file_name() != "." && entry.file_name() != ".." {
                return Ok(false);
            }
        }

        Ok(true)
    }
}
