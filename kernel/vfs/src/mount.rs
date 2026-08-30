use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use roxy_utils::Lock;

use crate::{AccessMode, FilePermissions, FileSystem, ResolvedPath, VfsError, file::ActiveHandles};

struct Mount {
    filesystem: Arc<dyn FileSystem>,
    active: Arc<ActiveHandles>,
}

pub(crate) struct ResolvedMount {
    pub(crate) mount_path: ResolvedPath,
    pub(crate) local_path: ResolvedPath,
    pub(crate) filesystem: Arc<dyn FileSystem>,
    pub(crate) active: Arc<ActiveHandles>,
}

pub struct Vfs {
    mounts: Lock<BTreeMap<ResolvedPath, Mount>>,
}

impl Vfs {
    #[must_use]
    pub fn new() -> Self {
        Self {
            mounts: Lock::new(BTreeMap::new()),
        }
    }

    pub fn mount(
        &self,
        path: ResolvedPath,
        filesystem: Arc<dyn FileSystem>,
    ) -> Result<(), VfsError> {
        let mut mounts = self.mounts.lock();

        if mounts.contains_key(&path) {
            return Err(VfsError::Busy);
        }

        mounts.insert(
            path,
            Mount {
                filesystem,
                active: Arc::new(ActiveHandles::default()),
            },
        );

        Ok(())
    }

    pub fn unmount(&self, path: &ResolvedPath) -> Result<Arc<dyn FileSystem>, VfsError> {
        let mut mounts = self.mounts.lock();
        let mount = mounts.get(path).ok_or(VfsError::NotFound)?;

        if mount.active.any() {
            return Err(VfsError::Busy);
        }

        let mount = mounts.remove(path).ok_or(VfsError::NotFound)?;

        Ok(mount.filesystem)
    }

    pub fn sync(&self) -> Result<(), VfsError> {
        let filesystems: Vec<_> = self
            .mounts
            .lock()
            .values()
            .map(|mount| mount.filesystem.clone())
            .collect();

        for filesystem in filesystems {
            filesystem.sync()?;
        }

        Ok(())
    }

    pub fn set_permissions(
        &self,
        path: &ResolvedPath,
        permissions: FilePermissions,
    ) -> Result<(), VfsError> {
        let resolved = self.resolve(path)?;

        resolved
            .filesystem
            .set_permissions(&resolved.local_path, permissions)
    }

    /// Checks whether the current process may access `path` for the requested `mode`.
    ///
    /// The caller's access rights are always those of root (real uid/gid are 0), so read and
    /// write are always permitted when the entry exists; only execute requires an execute bit.
    /// An empty mode slice tests only for existence (`F_OK`).
    pub fn access(&self, path: &ResolvedPath, mode: &[AccessMode]) -> Result<(), VfsError> {
        let metadata = self.metadata(path)?;

        if mode.contains(&AccessMode::Execute) && metadata.permissions.bits() & 0o111 == 0 {
            return Err(VfsError::PermissionDenied);
        }

        Ok(())
    }

    pub(crate) fn resolve(&self, path: &ResolvedPath) -> Result<ResolvedMount, VfsError> {
        let mounts = self.mounts.lock();
        let (mount_path, mount) = mounts
            .iter()
            .rev()
            .filter(|(mount_path, _)| mount_path.contains(path))
            .max_by_key(|(mount_path, _)| mount_path.as_bytes().len())
            .ok_or(VfsError::NotFound)?;

        Ok(ResolvedMount {
            mount_path: mount_path.clone(),
            local_path: path.relative_to(mount_path),
            filesystem: mount.filesystem.clone(),
            active: mount.active.clone(),
        })
    }
}

impl Default for Vfs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::sync::Arc;

    use crate::{OpenOptions, ResolvedPath, Vfs, VfsError, test_utils::MockFileSystem};

    roxy_test::kernel_test!(
        "roxy-vfs::routes-longest-mount-and-syncs-all",
        routes_longest_mount_and_syncs_all,
        {
            let vfs = Vfs::new();
            let root = Arc::new(MockFileSystem::new(1));
            let nested = Arc::new(MockFileSystem::new(2));

            vfs.mount(ResolvedPath::root(), root.clone()).unwrap();
            vfs.mount(path(b"/mnt"), nested.clone()).unwrap();

            assert_eq!(vfs.metadata(&path(b"/mnt/file")).unwrap().file_id, 2);
            assert_eq!(nested.last_path(), b"/file");
            assert_eq!(vfs.metadata(&path(b"/mnt2")).unwrap().file_id, 1);

            vfs.sync().unwrap();

            assert_eq!(root.sync_count(), 1);
            assert_eq!(nested.sync_count(), 1);
        }
    );

    roxy_test::kernel_test!(
        "roxy-vfs::active-handle-blocks-unmount",
        active_handle_blocks_unmount,
        {
            let vfs = Vfs::new();

            vfs.mount(ResolvedPath::root(), Arc::new(MockFileSystem::new(7)))
                .unwrap();

            let file = vfs.open(&path(b"/file"), OpenOptions::read_only()).unwrap();

            assert!(matches!(
                vfs.unmount(&ResolvedPath::root()),
                Err(VfsError::Busy)
            ));

            drop(file);

            vfs.unmount(&ResolvedPath::root()).unwrap();
        }
    );

    fn path(bytes: &[u8]) -> ResolvedPath {
        ResolvedPath::resolve(bytes).unwrap()
    }
}
