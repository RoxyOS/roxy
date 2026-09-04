use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use roxy_fd::{
    FileError, FileMetadata, IoctlError, IoctlRequest, MmapError, MmapTarget, PollEvents,
};
use roxy_poll::{PollListener, PollRegistration};
use roxy_utils::Lock;

/// A character device exposed through the device filesystem.
///
/// Implementations own device semantics and state; the device filesystem owns name lookup and
/// descriptor adaptation. A device is registered once and lives for the kernel lifetime.
pub trait Device: Send + Sync {
    /// Returns metadata for the device node.
    fn metadata(&self) -> FileMetadata;

    /// Returns the device instance an `open` of this node should hand out, or `None` to use `self`.
    ///
    /// A factory device (such as `/dev/ptmx`) overrides this so that opening the path yields a
    /// fresh instance rather than the node itself; ordinary devices return `None` and are opened as
    /// themselves.
    fn open(&self) -> Option<Arc<dyn Device>> {
        None
    }

    /// Reports whether this device is a terminal (for example a pty slave).
    fn is_terminal(&self) -> bool {
        false
    }

    /// Returns this terminal's openable pathname within the device filesystem (for example
    /// `/dev/tty0` or `/dev/pts/3`), when the device is a terminal.
    ///
    /// The path is the one the device was registered under (or, for dynamic devices such as pty
    /// slaves, is resolved from), so a `ttyname` consumer can reopen it. Non-terminal devices
    /// return `None`.
    fn terminal_path(&self) -> Option<Vec<u8>> {
        None
    }

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

    /// Registers a listener to be notified when this device's readiness may have changed.
    ///
    /// The default implementation returns an inactive registration (no-op). Devices that can
    /// become readable asynchronously (e.g. input devices) should override this.
    fn register_poll_listener(&self, _listener: Arc<PollListener>) -> PollRegistration {
        PollRegistration::inactive()
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
    /// Resolvers for device paths created dynamically at runtime (e.g. `/dev/pts/N` pty slaves);
    /// each resolver may own a distinct dynamic namespace, consulted in registration order.
    dynamic: Lock<Vec<Arc<dyn DynamicDeviceResolver>>>,
}

/// Resolves device paths that appear only at runtime (after registration).
///
/// The static registry names devices created during initialization; a dynamic resolver supplies
/// devices whose identity depends on runtime state, such as per-pair pty slave nodes. It is read
/// after the static table misses and never mutates the static registry.
pub trait DynamicDeviceResolver: Send + Sync {
    /// Returns the device for a mount-relative path, or `None` when it does not exist.
    fn resolve(&self, path: &[u8]) -> Option<Arc<dyn Device>>;
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
            dynamic: Lock::new(Vec::new()),
        }
    }

    /// Registers a dynamic path resolver consulted (in registration order) after the static table
    /// misses. The static table always wins, so static devices shadow dynamic ones.
    pub fn register_dynamic_resolver(&self, resolver: Arc<dyn DynamicDeviceResolver>) {
        self.dynamic.lock().push(resolver);
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

    /// Resolves a mount-relative path: the static registry first, then each dynamic resolver in
    /// registration order (first match wins).
    pub(crate) fn resolve(&self, path: &[u8]) -> Option<Arc<dyn Device>> {
        let static_device = self.devices.lock().get(path).cloned();
        if static_device.is_some() {
            return static_device;
        }

        let resolvers: Vec<_> = self.dynamic.lock().clone();
        for resolver in resolvers {
            if let Some(device) = resolver.resolve(path) {
                return Some(device);
            }
        }

        None
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
