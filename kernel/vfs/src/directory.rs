use alloc::vec::Vec;

use crate::{FileSystem, FileType, Vfs, VfsError, ResolvedPath};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirEntry {
    pub name: Vec<u8>,
    pub file_type: FileType,
}

impl Vfs {
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

    pub fn symlink(
        &self,
        target: impl AsRef<[u8]>,
        link: &ResolvedPath,
    ) -> Result<(), VfsError> {
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

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::sync::Arc;

    use crate::{OpenOptions, ResolvedPath, Vfs, VfsError, test_utils::MockFileSystem};

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

            let file = vfs
                .open(&path(b"/file"), OpenOptions::read_only())
                .unwrap();

            assert_eq!(vfs.unlink(&path(b"/file")), Err(VfsError::Busy));

            drop(file);

            vfs.unlink(&path(b"/file")).unwrap();
        }
    );

    fn path(bytes: &[u8]) -> ResolvedPath {
        ResolvedPath::resolve(bytes).unwrap()
    }
}
