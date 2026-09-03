#![no_std]

extern crate alloc;

use alloc::sync::Arc;

use heapless::Deque;
use roxy_devfs::Device;
use roxy_evdev_types::{
    EV_KEY, EV_LED, EV_SW, EV_SYN, EV_VERSION, EvdevCapabilities, EvdevDeviceId, InputEvent,
    encode_bits_bitmap,
};
use roxy_fd::{FileError, FileMetadata, FileType, IoctlError, IoctlRequest, PollEvents};
use roxy_poll::{PollListener, PollListeners, PollRegistration};
use roxy_utils::Lock;

/// Maximum number of queued events. The device owner queues serialised `InputEvent` records; a
/// keyboard producer queues two per key transition (the change plus its `SYN_REPORT` commit).
const EVENT_QUEUE_CAPACITY: usize = 256;

/// Static identity a generic evdev device exposes to user space and to the devfs registry.
#[derive(Clone, Copy)]
pub struct EvdevConfig {
    /// Stable devfs file ID for the node.
    pub file_id: u64,
    /// `EVIOCGNAME`: device name (no trailing NUL; written into a zeroed buffer).
    pub name: &'static [u8],
    /// `EVIOCGPHYS`: physical location string (no trailing NUL).
    pub phys: &'static [u8],
    /// `EVIOCGUNIQ`: unique id string (no trailing NUL).
    pub uniq: &'static [u8],
    /// `EVIOCGID`: bus type, vendor, product, version.
    pub id: EvdevDeviceId,
}

/// A generic evdev character device.
///
/// This core is deliberately free of any input *kind* (keyboard, pointer, …): it owns the event
/// queue, serves queued `InputEvent` records through `read`, answers the generic `EVIOC*`
/// queries, and reports per-device capabilities. Producers (e.g. `roxy-evdev-keyboard`)
/// construct one with an [`EvdevConfig`] and [`EvdevCapabilities`], then push encoded events via
/// [`EvdevDevice::push`].
pub struct EvdevDevice {
    queue: Lock<Deque<InputEvent, EVENT_QUEUE_CAPACITY>>,
    config: EvdevConfig,
    capabilities: EvdevCapabilities,
    grab: Lock<bool>,
    poll_listeners: Arc<PollListeners>,
}

impl EvdevDevice {
    /// Creates a core evdev device from the given identity and capabilities.
    #[must_use]
    pub fn create(config: EvdevConfig, capabilities: EvdevCapabilities) -> Arc<Self> {
        Arc::new(Self {
            queue: Lock::new(Deque::new()),
            config,
            capabilities,
            grab: Lock::new(false),
            poll_listeners: Arc::new(PollListeners::new()),
        })
    }

    /// Queues one serialised event and wakes blocked poll listeners.
    ///
    /// Called by the owning producer (e.g. the keyboard `KeyboardListener`) on every input
    /// arrival. The caller is responsible for filling the timestamp and the `type`/`code`/
    /// `value` fields of `event`.
    pub fn push(&self, event: InputEvent) {
        let mut queue = self.queue.lock();
        if queue.is_full() {
            // Drop the oldest queued event so the producer never blocks.
            let _ = queue.pop_front();
        }
        let _ = queue.push_back(event);
        drop(queue);
        self.poll_listeners.notify();
    }
}

impl Device for EvdevDevice {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: self.config.file_id,
            file_type: FileType::CharacterDevice,
            permissions: 0o600,
            size: 0,
            hard_links: 1,
        }
    }

    fn poll(&self) -> PollEvents {
        PollEvents {
            readable: !self.queue.lock().is_empty(),
            ..PollEvents::default()
        }
    }

    fn read(&self, output: &mut [u8]) -> Result<usize, FileError> {
        let mut queue = self.queue.lock();
        let mut written = 0;

        while written + 24 <= output.len() {
            let Some(event) = queue.pop_front() else {
                break;
            };
            // SAFETY: InputEvent's repr(C) layout is pinned by compile-time size/offset
            // assertions, so its object representation is exactly the 24-byte wire format.
            let bytes: [u8; 24] = unsafe { core::mem::transmute(event) };
            output[written..written + 24].copy_from_slice(&bytes);
            written += 24;
        }

        if written == 0 {
            Err(FileError::WouldBlock)
        } else {
            Ok(written)
        }
    }

    fn ioctl(&self, request: IoctlRequest<'_>) -> Result<(), IoctlError> {
        match request {
            IoctlRequest::EvdevGetVersion(version) => {
                *version = EV_VERSION;
                Ok(())
            }
            IoctlRequest::EvdevGetId(id) => {
                *id = self.config.id;
                Ok(())
            }
            IoctlRequest::EvdevGetName(buffer) => {
                copy_string(self.config.name, buffer);
                Ok(())
            }
            IoctlRequest::EvdevGetPhys(buffer) => {
                copy_string(self.config.phys, buffer);
                Ok(())
            }
            IoctlRequest::EvdevGetUniq(buffer) => {
                copy_string(self.config.uniq, buffer);
                Ok(())
            }
            IoctlRequest::EvdevGetRep(rep) => {
                // Default Linux repeat delay (250ms) and period (33ms).
                rep[0] = 250;
                rep[1] = 33;
                Ok(())
            }
            IoctlRequest::EvdevSetRep(_rep) => Ok(()),
            IoctlRequest::EvdevGetBits {
                ev,
                buffer,
                written,
            } => {
                let supported: &[u16] = match ev {
                    // EVIOCGBIT(0) queries the set of supported event *types*.
                    code if code == EV_SYN => self.capabilities.event_types,
                    code if code == EV_KEY => self.capabilities.key_codes,
                    code if code == EV_LED => self.capabilities.led_codes,
                    code if code == EV_SW => self.capabilities.switch_codes,
                    _ => &[],
                };
                *written = encode_bits_bitmap(supported, buffer);
                Ok(())
            }
            IoctlRequest::EvdevGrab(grab) => {
                *self.grab.lock() = grab;
                Ok(())
            }
            IoctlRequest::EvdevSetClockId(_clock_id) => Ok(()),
            _ => Err(IoctlError::NotTty),
        }
    }

    fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.poll_listeners.register(listener)
    }
}

/// Copies `source` into `buffer`, truncating when `buffer` is shorter. The remainder of a
/// caller-provided zeroed buffer stays zeroed, terminating the string.
fn copy_string(source: &[u8], buffer: &mut [u8]) {
    for (slot, byte) in buffer.iter_mut().zip(source.iter().copied()) {
        *slot = byte;
    }
}
