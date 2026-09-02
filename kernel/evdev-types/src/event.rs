//! Neutral evdev event and device types.
//!
//! These are plain kernel types, **not** `#[repr(C)]` userspace records, except for `InputEvent`
//! which is a device-serialised protocol record (see AGENTS.md "Design and Safety").

use core::mem::{offset_of, size_of};

/// A serialised `input_event` record as served to userspace through `read`.
///
/// This is a **device-serialised protocol record**, not a direct syscall ABI argument: the
/// evdev device fills it and copies it into the byte buffer returned by `read`, and the syscall
/// layer never inspects its layout. It therefore lives here rather than in `kernel/syscall`
/// (see AGENTS.md "Design and Safety").
///
/// Sourced from Linux `include/uapi/linux/input.h`: on `x86_64` the `timeval` is 16 bytes
/// (two `i64`), followed by `type`/`code` (`u16`) and `value` (`i32`), for a 24-byte record.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct InputEvent {
    pub tv_sec: i64,
    pub tv_usec: i64,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

const _: () = assert!(size_of::<InputEvent>() == 24);
const _: () = assert!(offset_of!(InputEvent, tv_sec) == 0);
const _: () = assert!(offset_of!(InputEvent, tv_usec) == 8);
const _: () = assert!(offset_of!(InputEvent, type_) == 16);
const _: () = assert!(offset_of!(InputEvent, code) == 18);
const _: () = assert!(offset_of!(InputEvent, value) == 20);

/// The `input_id` of the device (`EVIOCGID`).
///
/// Neutral representation of the four `__u16` fields; the syscall layer copies them into the
/// ABI record.
#[derive(Clone, Copy, Debug, Default)]
pub struct EvdevDeviceId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

/// Absolute-axis calibration info (`EVIOCGABS`/`EVIOCSABS`).
///
/// A keyboard has no absolute axes, so this is exposed for completeness and for the ioctl
/// surface to stay symmetric with Linux.
#[derive(Clone, Copy, Debug, Default)]
pub struct EvdevAbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

/// Static capability description for a generic evdev device.
///
/// The device owner (e.g. `roxy-evdev-keyboard`) provides these when constructing the core
/// `EvdevDevice`. The core uses them to answer `EVIOCGBIT` queries per-device rather than
/// relying on global keyboard-specific constants in the syscall layer.
#[derive(Clone, Copy)]
pub struct EvdevCapabilities {
    /// Event types the device supports (`EVIOCGBIT(0, …)`).
    ///
    /// Typically `[EV_SYN, EV_KEY]` for a keyboard.
    pub event_types: &'static [u16],
    /// Supported key codes (`EVIOCGBIT(EV_KEY, …)`).
    pub key_codes: &'static [u16],
    /// Supported LED codes (`EVIOCGBIT(EV_LED, …)`).
    pub led_codes: &'static [u16],
    /// Supported switch codes (`EVIOCGBIT(EV_SW, …)`).
    pub switch_codes: &'static [u16],
}
