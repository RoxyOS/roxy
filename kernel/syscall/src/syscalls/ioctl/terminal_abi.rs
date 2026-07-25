use core::{
    mem::{align_of, offset_of, size_of},
    slice,
};

use roxy_memory::UserAddress;
use roxy_tty_types::{LocalFlags, Termios, WindowSize};
use roxy_vm::AddrSpaceHandle;

use crate::errno::Errno;

pub(super) const TERMIOS_SIZE: usize = size_of::<TermiosAbi>();
pub(super) const WINDOW_SIZE: usize = size_of::<WindowSizeAbi>();

#[repr(C)]
struct TermiosAbi {
    input_flags: u32,
    output_flags: u32,
    control_flags: u32,
    local_flags: u32,
    line_discipline: u8,
    control_characters: [u8; 32],
    padding: [u8; 3],
    input_speed: u32,
    output_speed: u32,
}

const _: () = assert!(size_of::<TermiosAbi>() == 60);
const _: () = assert!(align_of::<TermiosAbi>() == 4);
const _: () = assert!(offset_of!(TermiosAbi, line_discipline) == 16);
const _: () = assert!(offset_of!(TermiosAbi, control_characters) == 17);
const _: () = assert!(offset_of!(TermiosAbi, input_speed) == 52);
const _: () = assert!(offset_of!(TermiosAbi, output_speed) == 56);

#[repr(C)]
struct WindowSizeAbi {
    rows: u16,
    columns: u16,
    pixel_width: u16,
    pixel_height: u16,
}

const _: () = assert!(size_of::<WindowSizeAbi>() == 8);
const _: () = assert!(align_of::<WindowSizeAbi>() == 2);
const _: () = assert!(offset_of!(WindowSizeAbi, rows) == 0);
const _: () = assert!(offset_of!(WindowSizeAbi, columns) == 2);
const _: () = assert!(offset_of!(WindowSizeAbi, pixel_width) == 4);
const _: () = assert!(offset_of!(WindowSizeAbi, pixel_height) == 6);

pub(super) fn read_termios(
    addrspace: &AddrSpaceHandle,
    address: UserAddress,
) -> Result<Termios, Errno> {
    let mut abi = TermiosAbi::zeroed();
    // SAFETY: TermiosAbi is repr(C), explicitly represents its padding, contains only integer
    // fields, and the slice does not outlive the uniquely borrowed value.
    let bytes = unsafe {
        slice::from_raw_parts_mut(
            core::ptr::from_mut(&mut abi).cast::<u8>(),
            size_of::<TermiosAbi>(),
        )
    };

    addrspace
        .read_bytes(address, bytes)
        .map_err(|_| Errno::Fault)?;

    Ok(abi.into())
}

pub(super) fn write_termios(
    addrspace: &AddrSpaceHandle,
    address: UserAddress,
    termios: Termios,
) -> Result<(), Errno> {
    let abi = TermiosAbi::from(termios);
    // SAFETY: TermiosAbi is repr(C), explicitly initializes its padding, contains only integer
    // fields, and the slice does not outlive the borrowed value.
    let bytes = unsafe {
        slice::from_raw_parts(
            core::ptr::from_ref(&abi).cast::<u8>(),
            size_of::<TermiosAbi>(),
        )
    };

    addrspace
        .write_bytes(address, bytes)
        .map_err(|_| Errno::Fault)
}

pub(super) fn read_window_size(
    addrspace: &AddrSpaceHandle,
    address: UserAddress,
) -> Result<WindowSize, Errno> {
    let mut abi = WindowSizeAbi::zeroed();
    // SAFETY: WindowSizeAbi is repr(C), contains only integer fields, and the slice does not
    // outlive the uniquely borrowed value.
    let bytes = unsafe {
        slice::from_raw_parts_mut(
            core::ptr::from_mut(&mut abi).cast::<u8>(),
            size_of::<WindowSizeAbi>(),
        )
    };

    addrspace
        .read_bytes(address, bytes)
        .map_err(|_| Errno::Fault)?;

    Ok(abi.into())
}

pub(super) fn write_window_size(
    addrspace: &AddrSpaceHandle,
    address: UserAddress,
    window_size: WindowSize,
) -> Result<(), Errno> {
    let abi = WindowSizeAbi::from(window_size);
    // SAFETY: WindowSizeAbi is repr(C), contains only initialized integer fields, and the slice
    // does not outlive the borrowed value.
    let bytes = unsafe {
        slice::from_raw_parts(
            core::ptr::from_ref(&abi).cast::<u8>(),
            size_of::<WindowSizeAbi>(),
        )
    };

    addrspace
        .write_bytes(address, bytes)
        .map_err(|_| Errno::Fault)
}

impl TermiosAbi {
    const fn zeroed() -> Self {
        Self {
            input_flags: 0,
            output_flags: 0,
            control_flags: 0,
            local_flags: 0,
            line_discipline: 0,
            control_characters: [0; 32],
            padding: [0; 3],
            input_speed: 0,
            output_speed: 0,
        }
    }
}

impl From<TermiosAbi> for Termios {
    fn from(abi: TermiosAbi) -> Self {
        Self {
            input_flags: abi.input_flags,
            output_flags: abi.output_flags,
            control_flags: abi.control_flags,
            local_flags: LocalFlags::from_bits_retain(abi.local_flags),
            line_discipline: abi.line_discipline,
            control_characters: abi.control_characters,
            input_speed: abi.input_speed,
            output_speed: abi.output_speed,
        }
    }
}

impl From<Termios> for TermiosAbi {
    fn from(termios: Termios) -> Self {
        Self {
            input_flags: termios.input_flags,
            output_flags: termios.output_flags,
            control_flags: termios.control_flags,
            local_flags: termios.local_flags.bits(),
            line_discipline: termios.line_discipline,
            control_characters: termios.control_characters,
            padding: [0; 3],
            input_speed: termios.input_speed,
            output_speed: termios.output_speed,
        }
    }
}

impl WindowSizeAbi {
    const fn zeroed() -> Self {
        Self {
            rows: 0,
            columns: 0,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

impl From<WindowSizeAbi> for WindowSize {
    fn from(abi: WindowSizeAbi) -> Self {
        Self {
            rows: abi.rows,
            columns: abi.columns,
            pixel_width: abi.pixel_width,
            pixel_height: abi.pixel_height,
        }
    }
}

impl From<WindowSize> for WindowSizeAbi {
    fn from(window_size: WindowSize) -> Self {
        Self {
            rows: window_size.rows,
            columns: window_size.columns,
            pixel_width: window_size.pixel_width,
            pixel_height: window_size.pixel_height,
        }
    }
}
