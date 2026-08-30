use alloc::{boxed::Box, vec::Vec};

use ext4plus::{file::File, inode::InodeMode};
use roxy_vfs::{
    CreationMode, DirEntry, FileHandle, FilePermissions, FileSystem, FileType, Metadata,
    OpenOptions, ResolvedPath, VfsError,
};

use crate::{Ext4FileSystem, error::map_ext4, file::Ext4File, metadata, metadata::map_file_type};

impl FileSystem for Ext4FileSystem {
    fn open(
        &self,
        path: &ResolvedPath,
        options: OpenOptions,
    ) -> Result<Box<dyn FileHandle>, VfsError> {
        let _mutation = self.mutation.lock();

        // With O_NOFOLLOW the final component is resolved without following, so a trailing
        // symbolic link is reported as ELOOP instead of being followed.
        let inode = match self.resolve_inode(path, !options.no_follow) {
            Ok(_) if options.creation == CreationMode::CreateNew => {
                return Err(VfsError::AlreadyExists);
            }
            Ok(inode) => {
                if options.no_follow
                    && map_file_type(inode.metadata().file_type) == FileType::Symlink
                {
                    return Err(VfsError::Loop);
                }
                inode
            }
            Err(VfsError::NotFound) if options.creation != CreationMode::OpenExisting => {
                self.create_regular(path, options.permissions)?;

                // A newly created file is never a symbolic link, so it is safe to follow.
                self.resolve_inode(path, true)?
            }
            Err(error) => return Err(error),
        };

        let mut file = File::open_inode(&self.filesystem, inode).map_err(map_ext4)?;

        if options.truncate {
            file.truncate(0).map_err(map_ext4)?;
        }

        if options.append {
            file.seek_to(file.inode().size_in_bytes())
                .map_err(map_ext4)?;
        }

        Ok(Box::new(Ext4File {
            file,
            filesystem: self.filesystem.clone(),
            options,
            mutation: self.mutation.clone(),
            device: self.device,
        }))
    }

    fn metadata(&self, path: &ResolvedPath, follow_symlink: bool) -> Result<Metadata, VfsError> {
        self.resolve_inode(path, follow_symlink)
            .map(|inode| metadata::from_inode(&inode))
    }

    fn read_dir(&self, path: &ResolvedPath) -> Result<Vec<DirEntry>, VfsError> {
        self.filesystem
            .read_dir(path.as_bytes())
            .map_err(map_ext4)?
            .map(|entry| {
                let entry = entry.map_err(map_ext4)?;

                Ok(DirEntry {
                    file_id: u64::from(entry.inode.get()),
                    name: entry.file_name().as_ref().into(),
                    file_type: metadata::map_file_type(entry.file_type().map_err(map_ext4)?),
                })
            })
            .collect()
    }

    fn mkdir(&self, path: &ResolvedPath, permissions: FilePermissions) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.mkdir_inner(path, permissions)
    }

    fn rmdir(&self, path: &ResolvedPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.rmdir_inner(path)
    }

    fn set_permissions(
        &self,
        path: &ResolvedPath,
        permissions: FilePermissions,
    ) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        let mut inode = self.resolve_inode(path, true)?;
        let current = inode.mode();
        let mode = InodeMode::from_bits_retain(current.bits() & !0o7777 | permissions.bits());

        inode.set_mode(mode).map_err(map_ext4)?;
        inode.write(&self.filesystem).map_err(map_ext4)?;
        self.device.flush().map_err(|_| VfsError::Io)
    }

    fn unlink(&self, path: &ResolvedPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.unlink_inner(path)
    }

    fn hard_link(&self, source: &ResolvedPath, destination: &ResolvedPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.hard_link_inner(source, destination)
    }

    fn symlink(&self, target: &[u8], link: &ResolvedPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.symlink_inner(target, link)
    }

    fn read_link(&self, path: &ResolvedPath) -> Result<Vec<u8>, VfsError> {
        self.filesystem
            .read_link(path.as_bytes())
            .map(|path| path.as_ref().into())
            .map_err(map_ext4)
    }

    fn rename(&self, source: &ResolvedPath, destination: &ResolvedPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.rename_inner(source, destination)
    }

    fn sync(&self) -> Result<(), VfsError> {
        self.device.flush().map_err(|_| VfsError::Io)
    }
}
