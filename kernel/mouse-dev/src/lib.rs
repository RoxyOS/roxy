#![no_std]

//! `/dev/mouse`: the Roxy mouse device.
//!
//! One hardware sample becomes exactly one [`RoxyMouseEvent`] record, so userspace reads motion,
//! wheel and button state as a single update instead of reassembling a multi-record batch. The
//! record is a device-serialised protocol record served through `read`, not a direct syscall ABI
//! argument (see AGENTS.md "Design and Safety"), so the syscall layer never inspects its layout.
//!
//! The record layout and the `ROXY_MOUSE_BTN_*` values are mirrored by
//! `sysdeps/roxy/include/roxy/mouse-dev.h` in the Roxy mlibc fork. The two sides are a
//! hand-maintained contract and must change together.

extern crate alloc;

use alloc::sync::Arc;
use core::mem::{offset_of, size_of};

use heapless::Deque;
use roxy_devfs::Device;
use roxy_fd::{FileError, FileMetadata, FileType, PollEvents};
use roxy_mouse_input::{MouseButton, MouseEvent, MouseListener};
use roxy_poll::{PollListener, PollListeners, PollRegistration};
use roxy_utils::Lock;

/// Stable file ID for `/dev/mouse` within the devfs mount.
const MOUSE_FILE_ID: u64 = 5;

/// Maximum number of queued samples. Producers (the PS/2 IRQ handler) never block: a full queue
/// drops its oldest record instead.
const EVENT_QUEUE_CAPACITY: usize = 256;

/// Left button bit in the `buttons` field.
pub const BUTTON_LEFT: u32 = 1 << 0;
/// Right button bit in the `buttons` field.
pub const BUTTON_RIGHT: u32 = 1 << 1;
/// Middle button bit in the `buttons` field.
pub const BUTTON_MIDDLE: u32 = 1 << 2;

/// One hardware sample as served to userspace through `read`.
///
/// Layout per `roxy/mouse-dev.h`: a realtime timestamp in nanoseconds, relative motion, the wheel
/// delta of this sample, and the button state after the sample. The struct has no implicit
/// padding, so its object representation is exactly the wire format.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoxyMouseEvent {
    /// `CLOCK_REALTIME` timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Relative motion to the right, in hardware counts.
    pub dx: i32,
    /// Relative motion downward, in hardware counts.
    pub dy: i32,
    /// Wheel steps of this sample, positive upward. Always zero without a wheel.
    pub wheel: i32,
    /// Button state after this sample, as `ROXY_MOUSE_BTN_*` bits.
    pub buttons: u32,
}

const RECORD_SIZE: usize = size_of::<RoxyMouseEvent>();

const _: () = assert!(RECORD_SIZE == 24);
const _: () = assert!(offset_of!(RoxyMouseEvent, timestamp_ns) == 0);
const _: () = assert!(offset_of!(RoxyMouseEvent, dx) == 8);
const _: () = assert!(offset_of!(RoxyMouseEvent, dy) == 12);
const _: () = assert!(offset_of!(RoxyMouseEvent, wheel) == 16);
const _: () = assert!(offset_of!(RoxyMouseEvent, buttons) == 20);

/// The `/dev/mouse` character device.
///
/// It implements [`MouseListener`] so the mouse manager hands it every hardware sample, and
/// [`Device`] so devfs serves the queued records through `read` and reports readiness through
/// `poll`.
pub struct MouseDevice {
    queue: Lock<Deque<RoxyMouseEvent, EVENT_QUEUE_CAPACITY>>,
    /// Button state carried between samples; a sample reports transitions, the record reports
    /// state.
    buttons: Lock<u32>,
    /// Whether the attached device can report wheel movement.
    wheel_supported: bool,
    poll_listeners: Arc<PollListeners>,
}

/// Creates the mouse device.
///
/// `wheel_supported` is `false` for a mouse without a wheel, in which case wheel movement is
/// dropped rather than reported. Returns the devfs `Device` registered as `/dev/mouse` and the
/// [`MouseListener`] that shares its queue. The caller keeps the listener alive for the kernel
/// lifetime.
#[must_use]
pub fn create(wheel_supported: bool) -> (Arc<dyn Device>, Arc<MouseDevice>) {
    let device = Arc::new(MouseDevice {
        queue: Lock::new(Deque::new()),
        buttons: Lock::new(0),
        wheel_supported,
        poll_listeners: Arc::new(PollListeners::new()),
    });
    let listener = device.clone();
    (device, listener)
}

impl MouseDevice {
    /// Queues one record and wakes blocked poll listeners.
    pub fn push(&self, event: RoxyMouseEvent) {
        let mut queue = self.queue.lock();
        if queue.is_full() {
            // Drop the oldest record so the producer never blocks.
            let _ = queue.pop_front();
        }
        let _ = queue.push_back(event);
        drop(queue);
        self.poll_listeners.notify();
    }
}

impl MouseListener for MouseDevice {
    /// Folds one hardware sample into a single [`RoxyMouseEvent`] and queues it.
    fn on_receive_input(&self, events: &[MouseEvent]) {
        let mut dx: i32 = 0;
        let mut dy: i32 = 0;
        let mut wheel: i32 = 0;

        let mut buttons = self.buttons.lock();
        for event in events {
            match *event {
                MouseEvent::Move { right, down } => {
                    dx = dx.saturating_add(right);
                    dy = dy.saturating_add(down);
                }
                MouseEvent::Scroll { up } => {
                    if self.wheel_supported {
                        wheel = wheel.saturating_add(up);
                    }
                }
                MouseEvent::ButtonPressed(button) => *buttons |= button_bit(button),
                MouseEvent::ButtonReleased(button) => *buttons &= !button_bit(button),
            }
        }
        let state = *buttons;
        drop(buttons);

        let now = roxy_time::realtime_time();
        self.push(RoxyMouseEvent {
            timestamp_ns: duration_to_nanos(now),
            dx,
            dy,
            wheel,
            buttons: state,
        });
    }
}

impl Device for MouseDevice {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: MOUSE_FILE_ID,
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
        if output.len() < RECORD_SIZE {
            // A short buffer cannot hold one whole record, and splitting a record across two
            // `read` calls would leave userspace parsing garbage.
            return Err(FileError::BadOperation);
        }

        let mut queue = self.queue.lock();
        let mut written = 0;

        while written + RECORD_SIZE <= output.len() {
            let Some(event) = queue.pop_front() else {
                break;
            };
            // SAFETY: RoxyMouseEvent is repr(C) with no implicit padding, so its object
            // representation is exactly the 24-byte wire format (pinned by the assertions above).
            let bytes: [u8; RECORD_SIZE] = unsafe { core::mem::transmute(event) };
            output[written..written + RECORD_SIZE].copy_from_slice(&bytes);
            written += RECORD_SIZE;
        }

        if written == 0 {
            Err(FileError::WouldBlock)
        } else {
            Ok(written)
        }
    }

    fn register_poll_listener(&self, listener: Arc<PollListener>) -> PollRegistration {
        self.poll_listeners.register(listener)
    }
}

/// Converts a realtime duration into the wire timestamp.
fn duration_to_nanos(duration: core::time::Duration) -> u64 {
    duration
        .as_nanos()
        .try_into()
        .expect("realtime time fits in u64 nanoseconds")
}

/// Maps a semantic [`MouseButton`] to its `ROXY_MOUSE_BTN_*` bit.
///
/// The match is exhaustive on purpose: adding a button to [`MouseButton`] without assigning it a
/// stable ABI bit fails to compile.
const fn button_bit(button: MouseButton) -> u32 {
    match button {
        MouseButton::Left => BUTTON_LEFT,
        MouseButton::Right => BUTTON_RIGHT,
        MouseButton::Middle => BUTTON_MIDDLE,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::{
        BUTTON_LEFT, BUTTON_MIDDLE, BUTTON_RIGHT, FileError, FileType, MouseButton, MouseEvent,
        MouseListener, RECORD_SIZE, create,
    };
    use roxy_test::kernel_test;

    kernel_test!("roxy-mouse-dev::metadata", reports_character_device, {
        let (device, _listener) = create(true);
        let metadata = device.metadata();
        assert_eq!(metadata.file_type, FileType::CharacterDevice);
        assert_eq!(metadata.file_id, 5);
    });

    kernel_test!(
        "roxy-mouse-dev::sample",
        folds_one_sample_into_one_record,
        {
            let (device, listener) = create(true);

            listener.on_receive_input(&[
                MouseEvent::Move { right: 3, down: -2 },
                MouseEvent::Scroll { up: 1 },
                MouseEvent::ButtonPressed(MouseButton::Left),
            ]);

            let mut output = [0u8; RECORD_SIZE * 2];
            let read = device.read(&mut output).expect("one queued sample");
            assert_eq!(read, RECORD_SIZE);
            assert_eq!(i32::from_le_bytes(output[8..12].try_into().unwrap()), 3);
            assert_eq!(i32::from_le_bytes(output[12..16].try_into().unwrap()), -2);
            assert_eq!(i32::from_le_bytes(output[16..20].try_into().unwrap()), 1);
            assert_eq!(
                u32::from_le_bytes(output[20..24].try_into().unwrap()),
                BUTTON_LEFT
            );

            assert!(matches!(
                device.read(&mut output),
                Err(FileError::WouldBlock)
            ));
        }
    );

    kernel_test!("roxy-mouse-dev::buttons", tracks_state_across_samples, {
        let (device, listener) = create(true);

        listener.on_receive_input(&[MouseEvent::ButtonPressed(MouseButton::Right)]);
        listener.on_receive_input(&[MouseEvent::ButtonPressed(MouseButton::Middle)]);
        listener.on_receive_input(&[MouseEvent::ButtonReleased(MouseButton::Right)]);

        let mut output = [0u8; RECORD_SIZE * 3];
        assert_eq!(device.read(&mut output).unwrap(), RECORD_SIZE * 3);
        let second = u32::from_le_bytes(
            output[RECORD_SIZE + 20..RECORD_SIZE + 24]
                .try_into()
                .unwrap(),
        );
        let third = u32::from_le_bytes(
            output[2 * RECORD_SIZE + 20..2 * RECORD_SIZE + 24]
                .try_into()
                .unwrap(),
        );
        assert_eq!(second, BUTTON_RIGHT | BUTTON_MIDDLE);
        assert_eq!(third, BUTTON_MIDDLE);
    });

    kernel_test!("roxy-mouse-dev::wheel", drops_scroll_without_wheel, {
        let (device, listener) = create(false);
        listener.on_receive_input(&[MouseEvent::Scroll { up: 1 }]);

        let mut output = [0u8; RECORD_SIZE];
        assert_eq!(device.read(&mut output).unwrap(), RECORD_SIZE);
        assert_eq!(i32::from_le_bytes(output[16..20].try_into().unwrap()), 0);
    });

    kernel_test!("roxy-mouse-dev::read", rejects_short_buffer, {
        let (device, _listener) = create(true);
        let mut short = [0u8; RECORD_SIZE - 1];
        assert!(matches!(
            device.read(&mut short),
            Err(FileError::BadOperation)
        ));
    });
}
