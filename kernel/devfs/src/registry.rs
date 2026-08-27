use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use roxy_fd::{
    FileError, FileMetadata, IoctlError, IoctlRequest, MmapError, MmapTarget, PollEvents,
};
use roxy_utils::Lock;

/// A character device exposed through the device filesystem.
///
/// Implementations own device semantics and state; the device filesystem owns name lookup and
/// descriptor adaptation. A device is registered once and lives for the kernel lifetime.
pub trait Device: Send + Sync {
    /// Returns metadata for the device node.
    fn metadata(&self) -> FileMetadata;

    /// Reports the events that are currently ready for this device.
    #[must_use]
    fn poll(&self) -> PollEvents {
        PollEvents {
            readable: true,
            writable: true,
            ..PollEvents::default()
        }
    }

    /// Performs a typed ioctl operation.
    ///
    /// # Errors
    ///
    /// Returns an operation-specific ioctl error.
    fn ioctl(&self, _request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        Err(IoctlError::NotTty)
    }

    /// Describes the physical memory backing a file-backed `mmap` of this device.
    ///
    /// # Errors
    ///
    /// Returns an error when the device does not support mapping or the range is invalid.
    fn mmap(&self, _size: usize, _offset: u64) -> Result<MmapTarget, MmapError> {
        Err(MmapError::Unsupported)
    }

    /// Reads data from the device.
    ///
    /// # Errors
    ///
    /// Returns an error when the device does not support reads.
    fn read(&self, _output: &mut [u8]) -> Result<usize, FileError> {
        Err(FileError::BadOperation)
    }

    /// Writes data to the device.
    ///
    /// # Errors
    ///
    /// Returns an error when the device does not support writes.
    fn write(&self, _input: &[u8]) -> Result<usize, FileError> {
        Err(FileError::BadOperation)
    }
}

/// Names device drivers under the device filesystem mount point.
pub struct DeviceRegistry {
    devices: Lock<BTreeMap<Vec<u8>, Arc<dyn Device>>>,
}

impl Default for DeviceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterError {
    AlreadyExists,
}

impl DeviceRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            devices: Lock::new(BTreeMap::new()),
        }
    }

    /// Registers a device under a mount-relative path such as `b"fb0"`.
    ///
    /// # Errors
    ///
    /// Returns an error when a device is already registered under the path.
    pub fn register(&self, path: &[u8], device: Arc<dyn Device>) -> Result<(), RegisterError> {
        let mut devices = self.devices.lock();

        if devices.contains_key(path) {
            return Err(RegisterError::AlreadyExists);
        }

        devices.insert(path.to_vec(), device);

        Ok(())
    }

    pub(crate) fn get(&self, path: &[u8]) -> Option<Arc<dyn Device>> {
        self.devices.lock().get(path).cloned()
    }

    pub(crate) fn names(&self) -> Vec<Vec<u8>> {
        self.devices.lock().keys().cloned().collect()
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{sync::Arc, vec};

    use roxy_fd::{FileMetadata, FileType};
    use roxy_test::kernel_test;

    use super::{Device, DeviceRegistry, RegisterError};

    struct Stub;

    impl Device for Stub {
        fn metadata(&self) -> FileMetadata {
            FileMetadata {
                file_id: 1,
                file_type: FileType::CharacterDevice,
                permissions: 0o600,
                size: 0,
                hard_links: 1,
            }
        }
    }

    kernel_test!("roxy-devfs::registry", registers_once_and_looks_up, {
        let registry = DeviceRegistry::new();

        registry.register(b"fb0", Arc::new(Stub)).unwrap();
        assert_eq!(
            registry.register(b"fb0", Arc::new(Stub)),
            Err(RegisterError::AlreadyExists)
        );
        assert!(registry.get(b"fb0").is_some());
        assert!(registry.get(b"input").is_none());
        assert_eq!(registry.names(), vec![b"fb0".to_vec()]);
    });
}
