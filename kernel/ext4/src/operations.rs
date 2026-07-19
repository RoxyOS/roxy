use alloc::{boxed::Box, vec::Vec};

use ext4plus::file::File;
use roxy_vfs::{
    CreationMode, DirEntry, FileHandle, FileSystem, Metadata, OpenOptions, VfsError, VfsPath,
};

use crate::{Ext4FileSystem, error::map_ext4, file::Ext4File, metadata};

impl FileSystem for Ext4FileSystem {
    fn open(&self, path: &VfsPath, options: OpenOptions) -> Result<Box<dyn FileHandle>, VfsError> {
        let _mutation = self.mutation.lock();

        let inode = match self.resolve_inode(path, true) {
            Ok(_) if options.creation == CreationMode::CreateNew => {
                return Err(VfsError::AlreadyExists);
            }
            Ok(inode) => inode,
            Err(VfsError::NotFound) if options.creation != CreationMode::OpenExisting => {
                self.create_regular(path)?;

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
            options,
            mutation: self.mutation.clone(),
            device: self.device,
        }))
    }

    fn metadata(&self, path: &VfsPath, follow_symlink: bool) -> Result<Metadata, VfsError> {
        self.resolve_inode(path, follow_symlink)
            .map(|inode| metadata::from_inode(&inode))
    }

    fn read_dir(&self, path: &VfsPath) -> Result<Vec<DirEntry>, VfsError> {
        self.filesystem
            .read_dir(path.as_bytes())
            .map_err(map_ext4)?
            .map(|entry| {
                let entry = entry.map_err(map_ext4)?;

                Ok(DirEntry {
                    name: entry.file_name().as_ref().into(),
                    file_type: metadata::map_file_type(entry.file_type().map_err(map_ext4)?),
                })
            })
            .collect()
    }

    fn mkdir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.mkdir_inner(path)
    }

    fn rmdir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.rmdir_inner(path)
    }

    fn unlink(&self, path: &VfsPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.unlink_inner(path)
    }

    fn hard_link(&self, source: &VfsPath, destination: &VfsPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.hard_link_inner(source, destination)
    }

    fn symlink(&self, target: &[u8], link: &VfsPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.symlink_inner(target, link)
    }

    fn read_link(&self, path: &VfsPath) -> Result<Vec<u8>, VfsError> {
        self.filesystem
            .read_link(path.as_bytes())
            .map(|path| path.as_ref().into())
            .map_err(map_ext4)
    }

    fn rename(&self, source: &VfsPath, destination: &VfsPath) -> Result<(), VfsError> {
        let _mutation = self.mutation.lock();

        self.rename_inner(source, destination)
    }

    fn sync(&self) -> Result<(), VfsError> {
        self.device.flush().map_err(|_| VfsError::Io)
    }
}
