use alloc::{sync::Arc, vec::Vec};

use crate::{FileSystem, FileType, Metadata, ResolvedPath, Vfs, VfsError, file::ActiveHandles};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirEntry {
    pub file_id: u64,
    pub name: Vec<u8>,
    pub file_type: FileType,
}

pub struct VfsDirectory {
    metadata: Metadata,
    entries: Vec<DirEntry>,
    active: Arc<ActiveHandles>,
}

impl VfsDirectory {
    #[must_use]
    pub const fn metadata(&self) -> Metadata {
        self.metadata
    }

    #[must_use]
    pub fn entries(&self) -> &[DirEntry] {
        &self.entries
    }
}

impl Vfs {
    pub fn open_dir(&self, path: &ResolvedPath) -> Result<VfsDirectory, VfsError> {
        let resolved = self.resolve(path)?;
        let metadata = resolved.filesystem.metadata(&resolved.local_path, true)?;

        if metadata.file_type != FileType::Directory {
            return Err(VfsError::NotDirectory);
        }

        let entries = resolved.filesystem.read_dir(&resolved.local_path)?;
        resolved.active.add(metadata.file_id);

        Ok(VfsDirectory {
            metadata,
            entries,
            active: resolved.active,
        })
    }

    pub fn read_dir(&self, path: &ResolvedPath) -> Result<Vec<DirEntry>, VfsError> {
        let resolved = self.resolve(path)?;

        resolved.filesystem.read_dir(&resolved.local_path)
    }

    pub fn mkdir(&self, path: &ResolvedPath) -> Result<(), VfsError> {
        self.with_path(path, |filesystem, local| filesystem.mkdir(local))
    }

    pub fn rmdir(&self, path: &ResolvedPath) -> Result<(), VfsError> {
        self.mutate_path(path, |filesystem, local| filesystem.rmdir(local))
    }

    pub fn unlink(&self, path: &ResolvedPath) -> Result<(), VfsError> {
        self.mutate_path(path, |filesystem, local| filesystem.unlink(local))
    }

    pub fn read_link(&self, path: &ResolvedPath) -> Result<Vec<u8>, VfsError> {
        self.with_path(path, |filesystem, local| filesystem.read_link(local))
    }

    pub fn symlink(&self, target: impl AsRef<[u8]>, link: &ResolvedPath) -> Result<(), VfsError> {
        let target = target.as_ref();

        if target.is_empty() || target.len() > ResolvedPath::MAX_LEN || target.contains(&0) {
            return Err(VfsError::InvalidPath);
        }

        let resolved = self.resolve(link)?;

        resolved.filesystem.symlink(target, &resolved.local_path)
    }

    pub fn hard_link(
        &self,
        source: &ResolvedPath,
        destination: &ResolvedPath,
    ) -> Result<(), VfsError> {
        self.two_paths(source, destination, |filesystem, from, to| {
            filesystem.hard_link(from, to)
        })
    }

    pub fn rename(
        &self,
        source: &ResolvedPath,
        destination: &ResolvedPath,
    ) -> Result<(), VfsError> {
        self.two_paths(source, destination, |filesystem, from, to| {
            filesystem.rename(from, to)
        })
    }

    fn mutate_path<T>(
        &self,
        path: &ResolvedPath,
        operation: impl FnOnce(&dyn FileSystem, &ResolvedPath) -> Result<T, VfsError>,
    ) -> Result<T, VfsError> {
        let resolved = self.resolve(path)?;
        let metadata = resolved.filesystem.metadata(&resolved.local_path, false)?;

        if resolved.active.contains(metadata.file_id) {
            return Err(VfsError::Busy);
        }

        operation(&*resolved.filesystem, &resolved.local_path)
    }

    fn with_path<T>(
        &self,
        path: &ResolvedPath,
        operation: impl FnOnce(&dyn FileSystem, &ResolvedPath) -> Result<T, VfsError>,
    ) -> Result<T, VfsError> {
        let resolved = self.resolve(path)?;

        operation(&*resolved.filesystem, &resolved.local_path)
    }

    fn two_paths<T>(
        &self,
        source: &ResolvedPath,
        destination: &ResolvedPath,
        operation: impl FnOnce(&dyn FileSystem, &ResolvedPath, &ResolvedPath) -> Result<T, VfsError>,
    ) -> Result<T, VfsError> {
        let from = self.resolve(source)?;
        let to = self.resolve(destination)?;

        if from.mount_path != to.mount_path {
            return Err(VfsError::CrossDevice);
        }

        let metadata = from.filesystem.metadata(&from.local_path, false)?;

        if from.active.contains(metadata.file_id) {
            return Err(VfsError::Busy);
        }

        match to.filesystem.metadata(&to.local_path, false) {
            Ok(metadata) if to.active.contains(metadata.file_id) => return Err(VfsError::Busy),
            Ok(_) | Err(VfsError::NotFound) => {}
            Err(error) => return Err(error),
        }

        operation(&*from.filesystem, &from.local_path, &to.local_path)
    }
}

impl Drop for VfsDirectory {
    fn drop(&mut self) {
        self.active.remove(self.metadata.file_id);
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{boxed::Box, sync::Arc};

    use roxy_fd::OpenFile;

    use crate::{OpenOptions, ResolvedPath, Vfs, VfsError, test_utils::MockFileSystem};

    roxy_test::kernel_test!(
        "roxy-vfs::active-directory-handle",
        active_directory_handle_blocks_mutation,
        {
            let vfs = Vfs::new();

            vfs.mount(ResolvedPath::root(), Arc::new(MockFileSystem::directory(7)))
                .unwrap();

            let directory = vfs.open_dir(&path(b"/")).unwrap();
            assert_eq!(directory.metadata().file_type, crate::FileType::Directory);
            let file = OpenFile::new(Box::new(directory));

            assert!(file.read_directory_entries(1).unwrap().is_empty());
            assert_eq!(vfs.rmdir(&path(b"/")), Err(VfsError::Busy));

            drop(file);
            vfs.rmdir(&path(b"/")).unwrap();
        }
    );

    roxy_test::kernel_test!(
        "roxy-vfs::rejects-cross-mount-operations",
        rejects_cross_mount_operations,
        {
            let vfs = Vfs::new();

            vfs.mount(ResolvedPath::root(), Arc::new(MockFileSystem::new(1)))
                .unwrap();
            vfs.mount(path(b"/mnt"), Arc::new(MockFileSystem::new(2)))
                .unwrap();

            assert_eq!(
                vfs.rename(&path(b"/source"), &path(b"/mnt/destination")),
                Err(VfsError::CrossDevice)
            );
            assert_eq!(
                vfs.hard_link(&path(b"/source"), &path(b"/mnt/destination")),
                Err(VfsError::CrossDevice)
            );
        }
    );

    roxy_test::kernel_test!(
        "roxy-vfs::active-handle-blocks-mutation",
        active_handle_blocks_mutation,
        {
            let vfs = Vfs::new();

            vfs.mount(ResolvedPath::root(), Arc::new(MockFileSystem::new(7)))
                .unwrap();

            let file = vfs.open(&path(b"/file"), OpenOptions::read_only()).unwrap();

            assert_eq!(vfs.unlink(&path(b"/file")), Err(VfsError::Busy));

            drop(file);

            vfs.unlink(&path(b"/file")).unwrap();
        }
    );

    fn path(bytes: &[u8]) -> ResolvedPath {
        ResolvedPath::resolve(bytes).unwrap()
    }
}
