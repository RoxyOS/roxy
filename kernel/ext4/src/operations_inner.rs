use ext4plus::{dir::Dir, path::PathBuf};
use roxy_vfs::{FileType, VfsError, VfsPath};

use crate::{Ext4FileSystem, error::map_ext4, metadata};

impl Ext4FileSystem {
    pub(crate) fn mkdir_inner(&self, path: &VfsPath) -> Result<(), VfsError> {
        match self.resolve_inode(path, false) {
            Ok(_) => return Err(VfsError::AlreadyExists),
            Err(VfsError::NotFound) => {}
            Err(error) => return Err(error),
        }

        let (mut parent, name) = self.parent(path)?;
        let inode = self.new_inode(FileType::Directory)?;
        let mut directory =
            Dir::init(self.filesystem.clone(), inode, parent.inode().index).map_err(map_ext4)?;

        parent.link(name, directory.inode_mut()).map_err(map_ext4)
    }

    pub(crate) fn rmdir_inner(&self, path: &VfsPath) -> Result<(), VfsError> {
        let inode = self.resolve_inode(path, false)?;

        if metadata::from_inode(&inode).file_type != FileType::Directory {
            return Err(VfsError::NotDirectory);
        }

        if !self.empty_directory(path)? {
            return Err(VfsError::DirectoryNotEmpty);
        }

        let (mut parent, name) = self.parent(path)?;

        // FIXME: ext4plus may not reclaim the directory inode and blocks. See ISSUES.md.
        let _remaining_inode = parent.unlink(name, inode).map_err(map_ext4)?;

        Ok(())
    }

    pub(crate) fn unlink_inner(&self, path: &VfsPath) -> Result<(), VfsError> {
        let inode = self.resolve_inode(path, false)?;

        if metadata::from_inode(&inode).file_type == FileType::Directory {
            return Err(VfsError::IsDirectory);
        }

        let (mut parent, name) = self.parent(path)?;

        // FIXME: ext4plus can treat an inline symlink target as block pointers. See ISSUES.md.
        let _remaining_inode = parent.unlink(name, inode).map_err(map_ext4)?;

        Ok(())
    }

    pub(crate) fn hard_link_inner(
        &self,
        source: &VfsPath,
        destination: &VfsPath,
    ) -> Result<(), VfsError> {
        let mut inode = self.resolve_inode(source, false)?;

        match self.resolve_inode(destination, false) {
            Ok(_) => return Err(VfsError::AlreadyExists),
            Err(VfsError::NotFound) => {}
            Err(error) => return Err(error),
        }

        if metadata::from_inode(&inode).file_type == FileType::Directory {
            return Err(VfsError::Unsupported);
        }

        let (mut parent, name) = self.parent(destination)?;

        parent.link(name, &mut inode).map_err(map_ext4)
    }

    pub(crate) fn symlink_inner(&self, target: &[u8], link: &VfsPath) -> Result<(), VfsError> {
        match self.resolve_inode(link, false) {
            Ok(_) => return Err(VfsError::AlreadyExists),
            Err(VfsError::NotFound) => {}
            Err(error) => return Err(error),
        }

        let target = PathBuf::try_from(target).map_err(|_| VfsError::InvalidPath)?;
        let (mut parent, name) = self.parent(link)?;

        self.filesystem
            .symlink(&mut parent, name, target, 0, 0, roxy_time::realtime_time())
            .map(|_| ())
            .map_err(map_ext4)
    }

    pub(crate) fn rename_inner(
        &self,
        source: &VfsPath,
        destination: &VfsPath,
    ) -> Result<(), VfsError> {
        if source == destination {
            return Ok(());
        }

        let mut inode = self.resolve_inode(source, false)?;
        let file_type = metadata::from_inode(&inode).file_type;
        let (mut source_parent, source_name) = self.parent(source)?;
        let (mut destination_parent, destination_name) = self.parent(destination)?;

        if file_type == FileType::Directory
            && source_parent.inode().index != destination_parent.inode().index
        {
            return Err(VfsError::Unsupported);
        }

        match self.resolve_inode(destination, false) {
            Ok(existing) => {
                let destination_type = metadata::from_inode(&existing).file_type;

                if existing.index == inode.index {
                    return Ok(());
                }

                if file_type == FileType::Directory && destination_type != FileType::Directory {
                    return Err(VfsError::NotDirectory);
                }

                if file_type != FileType::Directory && destination_type == FileType::Directory {
                    return Err(VfsError::IsDirectory);
                }

                if destination_type == FileType::Directory {
                    self.rmdir_inner(destination)?;
                } else {
                    self.unlink_inner(destination)?;
                }
            }
            Err(VfsError::NotFound) => {}
            Err(error) => return Err(error),
        }

        destination_parent
            .link(destination_name, &mut inode)
            .map_err(map_ext4)?;

        let _remaining_inode = source_parent.unlink(source_name, inode).map_err(map_ext4)?;

        Ok(())
    }
}
