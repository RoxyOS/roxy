#![no_std]

//! `/dev/keyboard`: the Roxy keyboard device.
//!
//! The device serialises every parsed key transition into a fixed-size [`RoxyKeyEvent`] record
//! and serves the queued records through `read`. Userspace parses the stream by record size; the
//! syscall layer never inspects the layout, because this is a device-serialised protocol record
//! rather than a direct syscall ABI argument (see AGENTS.md "Design and Safety").
//!
//! The record layout and the `ROXY_KEY_*` values are mirrored by
//! `sysdeps/roxy/include/roxy/keyboard-dev.h` in the Roxy mlibc fork. The two sides are a
//! hand-maintained contract and must change together.

extern crate alloc;

use alloc::sync::Arc;
use core::mem::{offset_of, size_of};

use heapless::Deque;
use roxy_devfs::Device;
use roxy_fd::{FileError, FileMetadata, FileType, PollEvents};
use roxy_keyboard_input::{KeyCode, KeyEvent, KeyState, KeyboardListener};
use roxy_poll::{PollListener, PollListeners, PollRegistration};
use roxy_utils::Lock;

/// Stable file ID for `/dev/keyboard` within the devfs mount.
const KEYBOARD_FILE_ID: u64 = 4;

/// Maximum number of queued key transitions. Producers (the PS/2 IRQ handler) never block: a
/// full queue drops its oldest record instead.
const EVENT_QUEUE_CAPACITY: usize = 256;

/// One key transition as served to userspace through `read`.
///
/// Layout per `roxy/keyboard-dev.h`: a realtime timestamp in nanoseconds, the `ROXY_KEY_*` code,
/// the press/release flag, and two explicitly named zero fields. The reserved fields exist to make
/// the padding explicit: the struct then has no implicit padding, so its object representation is
/// exactly the wire format.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoxyKeyEvent {
    /// `CLOCK_REALTIME` timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// The `ROXY_KEY_*` code of the physical key.
    pub code: u16,
    /// `1` when the key was pressed, `0` when it was released.
    pub pressed: u8,
    /// Always zero.
    pub reserved: u8,
    /// Always zero.
    pub reserved2: u32,
}

const RECORD_SIZE: usize = size_of::<RoxyKeyEvent>();

const _: () = assert!(RECORD_SIZE == 16);
const _: () = assert!(offset_of!(RoxyKeyEvent, timestamp_ns) == 0);
const _: () = assert!(offset_of!(RoxyKeyEvent, code) == 8);
const _: () = assert!(offset_of!(RoxyKeyEvent, pressed) == 10);
const _: () = assert!(offset_of!(RoxyKeyEvent, reserved) == 11);
const _: () = assert!(offset_of!(RoxyKeyEvent, reserved2) == 12);

/// The `/dev/keyboard` character device.
///
/// It implements [`KeyboardListener`] so the keyboard manager hands it every parsed transition,
/// and [`Device`] so devfs serves the queued records through `read` and reports readiness through
/// `poll`.
pub struct KeyboardDevice {
    queue: Lock<Deque<RoxyKeyEvent, EVENT_QUEUE_CAPACITY>>,
    poll_listeners: Arc<PollListeners>,
}

/// Creates the keyboard device.
///
/// Returns the devfs `Device` registered as `/dev/keyboard` and the [`KeyboardListener`] that
/// shares its queue. The caller keeps the listener alive for the kernel lifetime.
#[must_use]
pub fn create() -> (Arc<dyn Device>, Arc<KeyboardDevice>) {
    let device = Arc::new(KeyboardDevice {
        queue: Lock::new(Deque::new()),
        poll_listeners: Arc::new(PollListeners::new()),
    });
    let listener = device.clone();
    (device, listener)
}

impl KeyboardDevice {
    /// Queues one record and wakes blocked poll listeners.
    pub fn push(&self, event: RoxyKeyEvent) {
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

impl KeyboardListener for KeyboardDevice {
    /// Serialises one parsed key transition into a [`RoxyKeyEvent`] and queues it.
    fn on_recive_input(&self, key: KeyEvent) {
        let now = roxy_time::realtime_time();
        self.push(RoxyKeyEvent {
            timestamp_ns: duration_to_nanos(now),
            code: keycode_to_roxy(key.code),
            pressed: u8::from(matches!(key.state, KeyState::Pressed)),
            reserved: 0,
            reserved2: 0,
        });
    }
}

impl Device for KeyboardDevice {
    fn metadata(&self) -> FileMetadata {
        FileMetadata {
            file_id: KEYBOARD_FILE_ID,
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
            // SAFETY: RoxyKeyEvent is repr(C) with every padding byte named, so its object
            // representation is exactly the 16-byte wire format (pinned by the assertions
            // above).
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

/// Maps a semantic [`KeyCode`] to its `ROXY_KEY_*` wire value.
///
/// The match is exhaustive on purpose: adding a key to `KeyCode` without assigning it a stable
/// ABI value fails to compile, so the wire contract cannot drift silently. The numeric values are
/// the contract with `sysdeps/roxy/include/roxy/keyboard-dev.h`; keep both sides in step.
///
/// The table deliberately stays a single match over 104 keys: splitting it would need a wildcard
/// arm and lose that compile-time coverage.
#[allow(clippy::too_many_lines)]
const fn keycode_to_roxy(code: KeyCode) -> u16 {
    match code {
        // Control and editing keys.
        KeyCode::Escape => 0x0001,
        KeyCode::Backspace => 0x0002,
        KeyCode::Tab => 0x0003,
        KeyCode::Return => 0x0004,
        KeyCode::Insert => 0x0005,
        KeyCode::Delete => 0x0006,
        KeyCode::Home => 0x0007,
        KeyCode::End => 0x0008,
        KeyCode::PageUp => 0x0009,
        KeyCode::PageDown => 0x000A,
        KeyCode::Menu => 0x000B,

        // Letters.
        KeyCode::A => 0x0010,
        KeyCode::B => 0x0011,
        KeyCode::C => 0x0012,
        KeyCode::D => 0x0013,
        KeyCode::E => 0x0014,
        KeyCode::F => 0x0015,
        KeyCode::G => 0x0016,
        KeyCode::H => 0x0017,
        KeyCode::I => 0x0018,
        KeyCode::J => 0x0019,
        KeyCode::K => 0x001A,
        KeyCode::L => 0x001B,
        KeyCode::M => 0x001C,
        KeyCode::N => 0x001D,
        KeyCode::O => 0x001E,
        KeyCode::P => 0x001F,
        KeyCode::Q => 0x0020,
        KeyCode::R => 0x0021,
        KeyCode::S => 0x0022,
        KeyCode::T => 0x0023,
        KeyCode::U => 0x0024,
        KeyCode::V => 0x0025,
        KeyCode::W => 0x0026,
        KeyCode::X => 0x0027,
        KeyCode::Y => 0x0028,
        KeyCode::Z => 0x0029,

        // Digits.
        KeyCode::Digit0 => 0x0030,
        KeyCode::Digit1 => 0x0031,
        KeyCode::Digit2 => 0x0032,
        KeyCode::Digit3 => 0x0033,
        KeyCode::Digit4 => 0x0034,
        KeyCode::Digit5 => 0x0035,
        KeyCode::Digit6 => 0x0036,
        KeyCode::Digit7 => 0x0037,
        KeyCode::Digit8 => 0x0038,
        KeyCode::Digit9 => 0x0039,

        // Punctuation and space.
        KeyCode::Backquote => 0x003A,
        KeyCode::Minus => 0x003B,
        KeyCode::Equals => 0x003C,
        KeyCode::BracketLeft => 0x003D,
        KeyCode::BracketRight => 0x003E,
        KeyCode::Backslash => 0x003F,
        KeyCode::Semicolon => 0x0040,
        KeyCode::Apostrophe => 0x0041,
        KeyCode::Comma => 0x0042,
        KeyCode::Period => 0x0043,
        KeyCode::Slash => 0x0044,
        KeyCode::Space => 0x0045,

        // Modifiers and locks.
        KeyCode::LeftShift => 0x0050,
        KeyCode::RightShift => 0x0051,
        KeyCode::LeftCtrl => 0x0052,
        KeyCode::RightCtrl => 0x0053,
        KeyCode::LeftAlt => 0x0054,
        KeyCode::RightAlt => 0x0055,
        KeyCode::LeftSuper => 0x0056,
        KeyCode::RightSuper => 0x0057,
        KeyCode::CapsLock => 0x0058,
        KeyCode::ScrollLock => 0x0059,
        KeyCode::NumpadLock => 0x005A,

        // Function keys and system keys.
        KeyCode::F1 => 0x0070,
        KeyCode::F2 => 0x0071,
        KeyCode::F3 => 0x0072,
        KeyCode::F4 => 0x0073,
        KeyCode::F5 => 0x0074,
        KeyCode::F6 => 0x0075,
        KeyCode::F7 => 0x0076,
        KeyCode::F8 => 0x0077,
        KeyCode::F9 => 0x0078,
        KeyCode::F10 => 0x0079,
        KeyCode::F11 => 0x007A,
        KeyCode::F12 => 0x007B,
        KeyCode::PrintScreen => 0x007C,
        KeyCode::PauseBreak => 0x007D,

        // Arrow keys and the numeric keypad.
        KeyCode::ArrowUp => 0x008C,
        KeyCode::ArrowDown => 0x008D,
        KeyCode::ArrowLeft => 0x008E,
        KeyCode::ArrowRight => 0x008F,
        KeyCode::NumpadDivide => 0x0090,
        KeyCode::NumpadMultiply => 0x0091,
        KeyCode::NumpadSubtract => 0x0092,
        KeyCode::NumpadAdd => 0x0093,
        KeyCode::NumpadEnter => 0x0094,
        KeyCode::NumpadDecimal => 0x0095,
        KeyCode::Numpad0 => 0x0096,
        KeyCode::Numpad1 => 0x0097,
        KeyCode::Numpad2 => 0x0098,
        KeyCode::Numpad3 => 0x0099,
        KeyCode::Numpad4 => 0x009A,
        KeyCode::Numpad5 => 0x009B,
        KeyCode::Numpad6 => 0x009C,
        KeyCode::Numpad7 => 0x009D,
        KeyCode::Numpad8 => 0x009E,
        KeyCode::Numpad9 => 0x009F,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::{
        EVENT_QUEUE_CAPACITY, FileError, FileType, KeyCode, KeyEvent, KeyState, KeyboardListener,
        RECORD_SIZE, create,
    };
    use roxy_test::kernel_test;

    kernel_test!("roxy-keyboard-dev::metadata", reports_character_device, {
        let (device, _listener) = create();
        let metadata = device.metadata();
        assert_eq!(metadata.file_type, FileType::CharacterDevice);
        assert_eq!(metadata.file_id, 4);
        assert_eq!(metadata.permissions, 0o600);
    });

    kernel_test!("roxy-keyboard-dev::read", returns_whole_records, {
        let (device, listener) = create();

        let mut short = [0u8; RECORD_SIZE - 1];
        assert!(matches!(
            device.read(&mut short),
            Err(FileError::BadOperation)
        ));

        listener.on_recive_input(KeyEvent {
            code: KeyCode::A,
            state: KeyState::Pressed,
        });
        listener.on_recive_input(KeyEvent {
            code: KeyCode::A,
            state: KeyState::Released,
        });

        let mut output = [0u8; RECORD_SIZE * 4];
        let read = device.read(&mut output).expect("two queued records");
        assert_eq!(read, RECORD_SIZE * 2);
        assert_eq!(u16::from_le_bytes([output[8], output[9]]), 0x0010);
        assert_eq!(output[10], 1);
        assert_eq!(u16::from_le_bytes([output[24], output[25]]), 0x0010);
        assert_eq!(output[26], 0);

        assert!(matches!(
            device.read(&mut output),
            Err(FileError::WouldBlock)
        ));
    });

    kernel_test!("roxy-keyboard-dev::poll", reflects_queue_state, {
        let (device, listener) = create();
        assert!(!device.poll().readable);

        listener.on_recive_input(KeyEvent {
            code: KeyCode::Escape,
            state: KeyState::Pressed,
        });
        assert!(device.poll().readable);

        let mut output = [0u8; RECORD_SIZE];
        assert_eq!(device.read(&mut output).unwrap(), RECORD_SIZE);
        assert!(!device.poll().readable);
    });

    kernel_test!("roxy-keyboard-dev::queue", drops_oldest_when_full, {
        let (device, listener) = create();
        for _ in 0..=EVENT_QUEUE_CAPACITY {
            listener.on_recive_input(KeyEvent {
                code: KeyCode::B,
                state: KeyState::Pressed,
            });
        }

        let mut buffer = [0u8; RECORD_SIZE * (EVENT_QUEUE_CAPACITY + 1)];
        let read = device.read(&mut buffer).expect("queue is not empty");
        assert_eq!(read, RECORD_SIZE * EVENT_QUEUE_CAPACITY);
    });
}
