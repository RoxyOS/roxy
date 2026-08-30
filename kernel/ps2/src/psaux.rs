use alloc::sync::Arc;

use roxy_devfs::{Device, DeviceRegistry};
use roxy_fd::{FileError, FileMetadata, FileType, PollEvents};
use roxy_poll::{PollListener, PollListeners, PollRegistration};
use roxy_utils::Lock;
use spin::Once;

use crate::mouse::MouseInput;

/// Stable file ID for the PS/2 auxiliary device within the devfs mount.
const PSAUX_FILE_ID: u64 = 3;

/// Raw PS/2 mouse bytes delivered by IRQ12, shared with the `/dev/psaux` device.
static MOUSE_INPUT: Lock<MouseInput> = Lock::new(MouseInput::new());
/// Poll listeners blocked waiting for the mouse to become readable.
static POLL_LISTENERS: Once<Arc<PollListeners>> = Once::new();

/// The PS/2 auxiliary (`/dev/psaux`) character device.
///
/// The device is stateless: the byte queue and poll listeners are shared statics, so a single
/// unit instance serves every open of the node. Reads return raw PS/2 bytes; protocol parsing is
/// left to the consuming driver.
#[derive(Clone, Copy, Debug, Default)]
pub struct PsauxDevice;

impl Device for PsauxDevice {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: PSAUX_FILE_ID,
            file_type: FileType::CharacterDevice,
            // Read-only for root: the mouse is an input-only device with no writable interface.
            permissions: 0o400,
            size: 0,
            hard_links: 1,
        }
    }

    /// Reports the mouse as readable when raw bytes are queued.
    fn poll(&self) -> PollEvents {
        PollEvents {
            readable: !MOUSE_INPUT.lock().is_empty(),
            ..PollEvents::default()
        }
    }

    /// Copies queued raw mouse bytes into `output`.
    ///
    /// Returns `WouldBlock` when no bytes are available. Callers that wait with `select`/`poll`
    /// register a listener and are woken by IRQ12, so they only read once data is queued.
    fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        if output.is_empty() {
            return Ok(0);
        }

        let read = MOUSE_INPUT.lock().read_into(output);

        if read == 0 {
            Err(FileError::WouldBlock)
        } else {
            Ok(read)
        }
    }

    fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        POLL_LISTENERS
            .get()
            .expect("psaux poll listeners must be initialized")
            .register(listener)
    }
}

/// Registers the poll-listener collection used by `/dev/psaux`.
///
/// # Panics
///
/// Panics when the listeners are initialized more than once.
pub fn initialize_poll_listeners() {
    POLL_LISTENERS.call_once(|| Arc::new(PollListeners::new()));
}

/// Queues one raw mouse byte from IRQ12 and wakes any blocked poll listeners.
pub fn push_byte(byte: u8) {
    MOUSE_INPUT.lock().push(byte);
    notify_listeners();
}

/// Wakes poll listeners blocked on the mouse becoming readable.
fn notify_listeners() {
    if let Some(listeners) = POLL_LISTENERS.get() {
        listeners.notify();
    }
}

/// Registers `/dev/psaux` with the shared device registry.
///
/// The auxiliary device is registered whenever PS/2 is initialized; whether a mouse is actually
/// attached only affects whether reads ever become non-empty, not the node's presence.
///
/// # Panics
///
/// Panics when another device already registered the `psaux` path.
pub fn register_psaux(registry: &DeviceRegistry) {
    registry
        .register(b"psaux", Arc::new(PsauxDevice))
        .expect("psaux is registered exactly once");
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::{FileType, PollEvents};
    use roxy_test::kernel_test;

    use super::{PSAUX_FILE_ID, PsauxDevice, initialize_poll_listeners, push_byte};
    use roxy_devfs::Device;

    fn device() -> PsauxDevice {
        initialize_poll_listeners();
        PsauxDevice
    }

    kernel_test!("roxy-ps2::psaux-metadata", reports_character_device, {
        let metadata = device().metadata();
        assert_eq!(metadata.file_id, PSAUX_FILE_ID);
        assert_eq!(metadata.file_type, FileType::CharacterDevice);
        assert_eq!(metadata.permissions, 0o400);
        assert_eq!(metadata.size, 0);
    });

    kernel_test!("roxy-ps2::psaux-read", returns_queued_bytes, {
        let dev = device();
        assert!(matches!(
            dev.read(&mut [0; 4]),
            Err(roxy_fd::FileError::WouldBlock)
        ));

        push_byte(0x08);
        push_byte(0x01);
        push_byte(0x02);

        let mut output = [0u8; 4];
        assert_eq!(dev.read(&mut output).unwrap(), 3);
        assert_eq!(&output[..3], &[0x08, 0x01, 0x02]);

        // Queue drained, so it blocks again.
        assert!(matches!(
            dev.read(&mut output),
            Err(roxy_fd::FileError::WouldBlock)
        ));
    });

    kernel_test!("roxy-ps2::psaux-poll", reflects_queue_state, {
        let dev = device();
        assert_eq!(
            dev.poll(),
            PollEvents {
                readable: false,
                ..PollEvents::default()
            }
        );

        push_byte(0x08);
        assert_eq!(
            dev.poll(),
            PollEvents {
                readable: true,
                ..PollEvents::default()
            }
        );
    });
}
