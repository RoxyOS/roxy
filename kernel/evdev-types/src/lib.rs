#![no_std]
#![allow(dead_code)]

//! Kernel-neutral types for the evdev device, plus the Linux evdev ABI value set.
//!
//! This crate is the vocabulary shared between the generic `roxy-evdev` device, `roxy-fd`'s
//! typed ioctl surface, and the `roxy-syscall` ABI layer. It deliberately does **not** define
//! any `#[repr(C)]` direct syscall ABI record — those live only in `kernel/syscall/src/
//! syscalls/ioctl/evdev_abi.rs`. The one `#[repr(C)]` type here, `InputEvent`, is a
//! device-serialised protocol record served through `read`, not a direct syscall ABI argument
//! (see AGENTS.md "Design and Safety").
//!
//! Contents:
//!
//! - `event`: the neutral `InputEvent` wire record, the `EvdevDeviceId`/`EvdevAbsInfo` records
//!   that cross the typed `IoctlRequest` boundary, and the `EvdevCapabilities` that describe a
//!   device;
//! - `codes`: the Linux evdev ABI values (`EV_KEY`, `KEY_A`, `SYN_REPORT`, ...) that appear in
//!   the *payload* of an event or a capability bitmap. These are protocol data values, not ioctl
//!   request numbers (which stay in `roxy-syscall`);
//! - `bitmap`: helpers that encode a list of supported codes into the Linux capability bitmap
//!   layout returned by `EVIOCGBIT`.
//!
//! Keyboard-specific mapping (`KeyCode` ↔ `KEY_*`) is **not** here: it belongs to the keyboard
//! evdev device (`roxy-evdev-keyboard`).

mod bitmap;
mod codes;
mod event;

pub use bitmap::encode_bits_bitmap;
pub use codes::*;
pub use event::{EvdevAbsInfo, EvdevCapabilities, EvdevDeviceId, InputEvent};
