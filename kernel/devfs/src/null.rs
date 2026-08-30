use alloc::sync::Arc;

use roxy_fd::{FileError, FileMetadata, FileType};

use crate::{Device, DeviceRegistry};

/// The `/dev/null` sink: reads return EOF and writes are accepted and discarded.
///
/// The device has no state, hardware, or backing storage. A single shared instance serves
/// every open of the node, so the unit struct is `Clone`/`Copy`/`Default` for convenience.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullDevice;

/// Stable file ID for the null device within the devfs mount.
const NULL_FILE_ID: u64 = 2;

impl Device for NullDevice {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: NULL_FILE_ID,
            file_type: FileType::CharacterDevice,
            // World-readable/writable: reading yields EOF and writing discards data, so there
            // is no content or resource to protect. This matches the conventional 0666 mode
            // of `/dev/null` on Unix systems.
            permissions: 0o666,
            size: 0,
            hard_links: 1,
        }
    }

    /// Reports EOF immediately — reading yields zero bytes with no error.
    fn read(&self, _output: &mut [u8]) -> Result<usize, FileError> {
        Ok(0)
    }

    /// Accepts and discards all data, reporting the full write as successful.
    fn write(&self, input: &[u8]) -> Result<usize, FileError> {
        Ok(input.len())
    }
}

/// Registers `/dev/null` with the shared device registry.
///
/// The null device is always present regardless of hardware, so the composition root calls
/// this unconditionally during kernel initialization.
///
/// # Panics
///
/// Panics when another device already registered the `null` path.
pub fn register_null(registry: &DeviceRegistry) {
    registry
        .register(b"null", Arc::new(NullDevice))
        .expect("null is registered exactly once");
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::{sync::Arc, vec};

    use roxy_fd::FileType;
    use roxy_test::kernel_test;

    use super::{NULL_FILE_ID, NullDevice, register_null};
    use crate::{Device, DeviceRegistry};

    kernel_test!("roxy-devfs::null-read", returns_eof, {
        let device = NullDevice;
        let mut buffer = [0xabu8; 16];
        assert_eq!(device.read(&mut buffer).unwrap(), 0);
        // Nothing is written, so the buffer is untouched.
        assert_eq!(buffer, [0xabu8; 16]);
    });

    kernel_test!("roxy-devfs::null-write", discards_all_data, {
        let device = NullDevice;
        assert_eq!(device.write(b"hello world").unwrap(), 11);
        assert_eq!(device.write(&[]).unwrap(), 0);
        assert_eq!(device.write(&[0u8; 256]).unwrap(), 256);
    });

    kernel_test!("roxy-devfs::null-metadata", reports_character_device, {
        let metadata = NullDevice.metadata();
        assert_eq!(metadata.file_id, NULL_FILE_ID);
        assert_eq!(metadata.file_type, FileType::CharacterDevice);
        assert_eq!(metadata.permissions, 0o666);
        assert_eq!(metadata.size, 0);
        assert_eq!(metadata.hard_links, 1);
    });

    kernel_test!("roxy-devfs::null-registration", registers_in_registry, {
        let registry = DeviceRegistry::new();
        register_null(&registry);

        assert!(registry.get(b"null").is_some());
        assert_eq!(registry.names(), vec![b"null".to_vec()]);
    });

    kernel_test!("roxy-devfs::null-registration-twice", rejects_duplicate, {
        let registry = DeviceRegistry::new();
        register_null(&registry);
        // A second registration must fail.
        assert!(registry.register(b"null", Arc::new(NullDevice)).is_err());
    });
}
