//! Linux evdev ABI records and EVIOC request-number constants.
//!
//! This module owns the `#[repr(C)]` **direct syscall ABI** records that cross the ioctl
//! boundary as marshalled arguments (`InputIdAbi`, `InputAbsInfoAbi`), their size/offset
//! assertions, and the request-number constants for every `EVIOC*` ioctl. The event stream
//! record `input_event` is a device-serialised protocol record (not direct syscall ABI), so it
//! lives in `roxy-evdev-types`; the device serialises it and the syscall layer never inspects
//! its layout.

use core::mem::{offset_of, size_of};

/// Linux `struct input_id` as seen by `x86_64` userspace (8 bytes).
#[repr(C)]
pub(super) struct InputIdAbi {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

const _: () = assert!(size_of::<InputIdAbi>() == 8);

/// Linux `struct input_absinfo` as seen by `x86_64` userspace (24 bytes).
#[repr(C)]
pub(super) struct InputAbsInfoAbi {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

const _: () = assert!(size_of::<InputAbsInfoAbi>() == 24);
const _: () = assert!(offset_of!(InputAbsInfoAbi, value) == 0);
const _: () = assert!(offset_of!(InputAbsInfoAbi, resolution) == 20);

// ── EVIOC request-number layout ─────────────────────────────────────────────
//
// `EVIOC*` requests are built with the Linux `_IOR`/`_IOW`/`_IOC` macros
// (`include/uapi/asm-generic/ioctl.h`):
//   _IOC(dir, type, nr, size) = (dir << 30) | (size << 16) | (type << 8) | nr
// with `_IOC_READ = 2`, `_IOC_WRITE = 1`. The type byte is always `'E'` (0x45). The `nr`
// byte identifies the operation; `EVIOCGNAME(len)` etc. additionally pack the buffer size into
// bits 16–29, so they are matched by `nr` rather than by exact constant.

/// The direction bits (bits 30–31).
const DIR_MASK: u64 = 3 << 30;
/// `_IOC_READ` (ioctl writes data out to the caller).
const DIR_READ: u64 = 2 << 30;

/// Matches every `EVIOC*` request by checking the type byte is `'E'`.
pub(super) fn is_evioc_request(request: u64) -> bool {
    (request >> 8) & 0xff == 0x45
}

/// Extracts the `nr` byte (bits 0–7) from an `EVIOC*` request.
pub(super) fn evioc_nr(request: u64) -> u8 {
    (request & 0xff) as u8
}

/// Extracts the size field (bits 16–29) from an `EVIOC*` request.
pub(super) fn evioc_size(request: u64) -> usize {
    ((request >> 16) & 0x3fff) as usize
}

/// Tests whether a request has the read (`_IOC_READ`) direction bit set.
pub(super) fn is_read_direction(request: u64) -> bool {
    request & DIR_MASK == DIR_READ
}
