use alloc::{boxed::Box, sync::Arc, vec::Vec};

use roxy_fd::{
    FileError, FileMetadata, FileType as FdFileType, IoctlError, IoctlRequest, MmapError,
    MmapTarget, PollEvents,
};
use roxy_poll::{PollListener, PollRegistration};
use roxy_vfs::{
    DirEntry, FileHandle, FilePermissions, FileSystem, Metadata, OpenOptions, ResolvedPath,
    SeekFrom, VfsError,
};

use crate::{Device, DeviceRegistry};

const DEVFS_ROOT: Metadata = Metadata {
    file_id: 1,
    file_type: roxy_vfs::FileType::Directory,
    permissions: FilePermissions::DEFAULT_DIRECTORY,
    size: 0,
    hard_links: 1,
};

/// A read-only pseudo filesystem exposing registered character devices.
///
/// The mount root is a directory listing every registered device; each leaf path opens the
/// corresponding `Device`. Namespace mutation is rejected because the device set is fixed by
/// registration, not by filesystem operations.
pub struct DevFs {
    registry: Arc<DeviceRegistry>,
}

impl DevFs {
    #[must_use]
    pub fn new(registry: Arc<DeviceRegistry>) -> Self {
        Self { registry }
    }
}

impl FileSystem for DevFs {
    fn open(
        &self,
        path: &ResolvedPath,
        _options: OpenOptions,
    ) -> Result<Box<dyn FileHandle>, VfsError> {
        let device = self
            .registry
            .get(path.as_bytes())
            .ok_or(VfsError::NotFound)?;

        Ok(Box::new(DeviceFile::new(device)))
    }

    fn metadata(&self, path: &ResolvedPath, _follow_symlink: bool) -> Result<Metadata, VfsError> {
        if path.is_root() {
            return Ok(DEVFS_ROOT);
        }

        let device = self
            .registry
            .get(path.as_bytes())
            .ok_or(VfsError::NotFound)?;

        Ok(to_vfs_metadata(device.metadata()))
    }

    fn read_dir(&self, path: &ResolvedPath) -> Result<Vec<DirEntry>, VfsError> {
        if !path.is_root() {
            return Err(VfsError::NotDirectory);
        }

        self.registry
            .names()
            .into_iter()
            .map(|name| {
                let device = self
                    .registry
                    .get(&name)
                    .expect("listed device is still registered");
                let metadata = device.metadata();

                Ok(DirEntry {
                    file_id: metadata.file_id,
                    name,
                    file_type: map_file_type(metadata.file_type),
                })
            })
            .collect()
    }

    fn mkdir(&self, _path: &ResolvedPath, _permissions: FilePermissions) -> Result<(), VfsError> {
        Err(VfsError::ReadOnly)
    }

    fn rmdir(&self, _path: &ResolvedPath) -> Result<(), VfsError> {
        Err(VfsError::ReadOnly)
    }

    fn unlink(&self, _path: &ResolvedPath) -> Result<(), VfsError> {
        Err(VfsError::ReadOnly)
    }

    fn hard_link(
        &self,
        _source: &ResolvedPath,
        _destination: &ResolvedPath,
    ) -> Result<(), VfsError> {
        Err(VfsError::ReadOnly)
    }

    fn symlink(&self, _target: &[u8], _link: &ResolvedPath) -> Result<(), VfsError> {
        Err(VfsError::ReadOnly)
    }

    fn read_link(&self, _path: &ResolvedPath) -> Result<Vec<u8>, VfsError> {
        Err(VfsError::NotFound)
    }

    fn rename(&self, _source: &ResolvedPath, _destination: &ResolvedPath) -> Result<(), VfsError> {
        Err(VfsError::ReadOnly)
    }

    fn set_permissions(
        &self,
        _path: &ResolvedPath,
        _permissions: FilePermissions,
    ) -> Result<(), VfsError> {
        Err(VfsError::ReadOnly)
    }

    fn sync(&self) -> Result<(), VfsError> {
        Ok(())
    }
}

/// Adapts one registered device to the VFS file-handle contract.
struct DeviceFile {
    device: Arc<dyn Device>,
}

impl DeviceFile {
    fn new(device: Arc<dyn Device>) -> Self {
        Self { device }
    }
}

impl FileHandle for DeviceFile {
    fn read(&mut self, destination: &mut [u8]) -> Result<usize, VfsError> {
        self.device.read(destination).map_err(map_device_error)
    }

    fn write(&mut self, source: &[u8]) -> Result<usize, VfsError> {
        self.device.write(source).map_err(map_device_error)
    }

    fn seek(&mut self, _position: SeekFrom) -> Result<u64, VfsError> {
        Err(VfsError::Unsupported)
    }

    fn truncate(&mut self, _size: u64) -> Result<(), VfsError> {
        Err(VfsError::Unsupported)
    }

    fn metadata(&self) -> Result<Metadata, VfsError> {
        Ok(to_vfs_metadata(self.device.metadata()))
    }

    fn sync(&mut self) -> Result<(), VfsError> {
        Ok(())
    }

    fn set_permissions(&mut self, _permissions: FilePermissions) -> Result<(), VfsError> {
        Err(VfsError::ReadOnly)
    }

    fn ioctl(&mut self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        self.device.ioctl(request)
    }

    fn mmap(&mut self, size: usize, offset: u64) -> Result<MmapTarget, MmapError> {
        self.device.mmap(size, offset)
    }

    fn poll(&self) -> Result<PollEvents, VfsError> {
        Ok(self.device.poll())
    }

    fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.device.register_poll_listener(listener)
    }
}

fn to_vfs_metadata(metadata: FileMetadata) -> Metadata {
    Metadata {
        file_id: metadata.file_id,
        file_type: map_file_type(metadata.file_type),
        permissions: FilePermissions::new(metadata.permissions)
            .expect("device reports valid permission bits"),
        size: metadata.size,
        hard_links: metadata.hard_links,
    }
}

fn map_file_type(file_type: FdFileType) -> roxy_vfs::FileType {
    match file_type {
        FdFileType::Regular => roxy_vfs::FileType::Regular,
        FdFileType::Directory => roxy_vfs::FileType::Directory,
        FdFileType::Symlink => roxy_vfs::FileType::Symlink,
        FdFileType::BlockDevice => roxy_vfs::FileType::BlockDevice,
        FdFileType::CharacterDevice => roxy_vfs::FileType::CharacterDevice,
        FdFileType::Fifo => roxy_vfs::FileType::Fifo,
        FdFileType::Socket => roxy_vfs::FileType::Socket,
        FdFileType::Unknown => roxy_vfs::FileType::Unknown,
    }
}

fn map_device_error(error: FileError) -> VfsError {
    match error {
        FileError::BadOperation => VfsError::Unsupported,
        FileError::WouldBlock => VfsError::WouldBlock,
        FileError::Io | FileError::BrokenPipe | FileError::NotConnected => VfsError::Io,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::sync::Arc;

    use roxy_fd::{FileMetadata, FileType, IoctlRequest, WindowSize};
    use roxy_test::kernel_test;
    use roxy_vfs::{FileType as VfsFileType, ResolvedPath, Vfs, VfsError};

    use super::DevFs;
    use crate::{Device, DeviceRegistry};

    struct Stub;

    impl Device for Stub {
        fn metadata(&self) -> FileMetadata {
            FileMetadata {
                file_id: 42,
                file_type: FileType::CharacterDevice,
                permissions: 0o600,
                size: 0,
                hard_links: 1,
            }
        }
    }

    fn filesystem() -> (Vfs, Arc<DeviceRegistry>) {
        let registry = Arc::new(DeviceRegistry::new());
        registry.register(b"fb0", Arc::new(Stub)).unwrap();
        let vfs = Vfs::new();
        // Mount below the root so lookups exercise mount-relative registry paths,
        // matching the real `/dev` mount in kernel-main.
        vfs.mount(
            ResolvedPath::resolve(b"/dev").unwrap(),
            Arc::new(DevFs::new(registry.clone())),
        )
        .unwrap();

        (vfs, registry)
    }

    kernel_test!("roxy-devfs::open-device", opens_registered_device, {
        let (vfs, _) = filesystem();
        let mut file = vfs
            .open(
                &ResolvedPath::resolve(b"/dev/fb0").unwrap(),
                roxy_vfs::OpenOptions::read_only(),
            )
            .unwrap();

        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.file_id, 42);
        assert_eq!(metadata.file_type, VfsFileType::CharacterDevice);
        assert_eq!(metadata.permissions.bits(), 0o600);
        assert_eq!(file.read(&mut [0; 4]), Err(VfsError::Unsupported));
        assert_eq!(
            file.seek(roxy_vfs::SeekFrom::Start(0)),
            Err(VfsError::Unsupported)
        );
        assert!(
            file.ioctl(IoctlRequest::GetWindowSize(&mut WindowSize::default()))
                .is_err()
        );
    });

    kernel_test!("roxy-devfs::missing-device", rejects_unknown_device, {
        let (vfs, _) = filesystem();
        let missing = ResolvedPath::resolve(b"/dev/tty0").unwrap();

        assert!(
            vfs.open(&missing, roxy_vfs::OpenOptions::read_only())
                .is_err()
        );
        assert_eq!(vfs.metadata(&missing), Err(VfsError::NotFound));
    });

    kernel_test!("roxy-devfs::root-directory", lists_registered_devices, {
        let (vfs, _) = filesystem();

        assert_eq!(
            vfs.metadata(&ResolvedPath::resolve(b"/dev").unwrap())
                .unwrap()
                .file_type,
            VfsFileType::Directory
        );
        let entries = vfs
            .read_dir(&ResolvedPath::resolve(b"/dev").unwrap())
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, b"fb0");
        assert_eq!(entries[0].file_type, VfsFileType::CharacterDevice);
        assert_eq!(
            vfs.read_dir(&ResolvedPath::resolve(b"/dev/fb0").unwrap()),
            Err(VfsError::NotDirectory)
        );
        assert_eq!(
            vfs.mkdir(
                &ResolvedPath::resolve(b"/dev/new").unwrap(),
                roxy_vfs::FilePermissions::DEFAULT_DIRECTORY
            ),
            Err(VfsError::ReadOnly)
        );
    });
}
