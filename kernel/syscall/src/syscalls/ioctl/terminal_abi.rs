use core::mem::{align_of, offset_of, size_of};

use roxy_memory::UserAddress;
use roxy_tty_types::{LocalFlags, Termios, WindowSize};

use crate::{
    args::{Out, SyscallArg, user_memory},
    errno::Errno,
};

#[repr(C)]
pub(super) struct TermiosAbi {
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

impl SyscallArg for TermiosAbi {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut abi = Self::zeroed();

        // SAFETY: TermiosAbi's checked repr(C) layout explicitly represents all padding, contains
        // only integers, and accepts every bit pattern.
        unsafe { user_memory::read(address, &mut abi) }?;

        Ok(abi)
    }
}

#[repr(C)]
pub(super) struct WindowSizeAbi {
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

impl SyscallArg for WindowSizeAbi {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut abi = Self::zeroed();

        // SAFETY: WindowSizeAbi's checked repr(C) layout contains only u16 fields without padding,
        // and every bit pattern is valid.
        unsafe { user_memory::read(address, &mut abi) }?;

        Ok(abi)
    }
}

pub(super) fn read_termios(address: UserAddress) -> Result<Termios, Errno> {
    let abi = TermiosAbi::parse(address.as_u64(), Errno::Fault)?;

    Ok(abi.into())
}

pub(super) fn write_termios(output: Out<TermiosAbi>, termios: Termios) -> Result<(), Errno> {
    let abi = TermiosAbi::from(termios);

    // SAFETY: TermiosAbi's checked repr(C) layout explicitly represents and initializes all
    // padding and contains only integer fields.
    unsafe { output.write(&abi) }
}

pub(super) fn read_window_size(address: UserAddress) -> Result<WindowSize, Errno> {
    let abi = WindowSizeAbi::parse(address.as_u64(), Errno::Fault)?;

    Ok(abi.into())
}

pub(super) fn write_window_size(
    output: Out<WindowSizeAbi>,
    window_size: WindowSize,
) -> Result<(), Errno> {
    let abi = WindowSizeAbi::from(window_size);

    // SAFETY: WindowSizeAbi's checked repr(C) layout contains only initialized u16 fields without
    // padding.
    unsafe { output.write(&abi) }
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
