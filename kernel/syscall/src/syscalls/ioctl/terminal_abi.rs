use core::mem::{align_of, offset_of, size_of};

use roxy_memory::UserAddress;
use roxy_tty_types::{TerminalAttributes, TerminalFlags, WindowSize};

use crate::{
    args::{Out, SyscallArg, user_memory},
    errno::Errno,
};

/// Fixed-layout terminal-attribute payload copied across the userspace syscall ABI.
///
/// Layout per `sysdeps/roxy/sysdeps/ioctl.cpp`; sizes pinned by the assertions below.
#[repr(C)]
pub(super) struct TerminalAttributesAbi {
    /// Terminal behavior flags, one [`TerminalFlags`] bit each.
    flags: u32,
    /// The interrupt character (`VINTR`), conventionally Ctrl+C.
    interrupt_byte: u8,
    /// The erase character (`VERASE`), conventionally backspace.
    erase_byte: u8,
    /// Always zero; names the bytes the record's alignment leaves implicit.
    reserved: u16,
}

const _: () = assert!(size_of::<TerminalAttributesAbi>() == 8);
const _: () = assert!(align_of::<TerminalAttributesAbi>() == 4);
const _: () = assert!(offset_of!(TerminalAttributesAbi, flags) == 0);
const _: () = assert!(offset_of!(TerminalAttributesAbi, interrupt_byte) == 4);
const _: () = assert!(offset_of!(TerminalAttributesAbi, erase_byte) == 5);
const _: () = assert!(offset_of!(TerminalAttributesAbi, reserved) == 6);

impl TerminalAttributesAbi {
    const fn zeroed() -> Self {
        Self {
            flags: 0,
            interrupt_byte: 0,
            erase_byte: 0,
            reserved: 0,
        }
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

pub(super) fn read_attributes(address: UserAddress) -> Result<TerminalAttributes, Errno> {
    let mut abi = TerminalAttributesAbi::zeroed();

    // SAFETY: TerminalAttributesAbi's checked repr(C) layout explicitly represents all padding,
    // contains only integers, and accepts every bit pattern.
    unsafe { user_memory::read(address, &mut abi) }?;

    let flags = decode_flags(abi.flags)?;

    Ok(TerminalAttributes {
        flags,
        interrupt_byte: abi.interrupt_byte,
        erase_byte: abi.erase_byte,
    })
}

pub(super) fn write_attributes(
    output: Out<TerminalAttributesAbi>,
    attributes: TerminalAttributes,
) -> Result<(), Errno> {
    let abi = TerminalAttributesAbi {
        flags: attributes.flags.bits(),
        interrupt_byte: attributes.interrupt_byte,
        erase_byte: attributes.erase_byte,
        reserved: 0,
    };

    // SAFETY: TerminalAttributesAbi's checked repr(C) layout explicitly represents and initializes
    // all padding and contains only integer fields.
    unsafe { output.write(&abi) }
}

/// Decodes one record's flag word, reporting a bit that names no attribute this kernel serves.
///
/// The word is a field of Roxy's own record rather than a word a caller shares with another
/// personality, so no value below a base can arrive and there is no foreign numbering to separate
/// from ours: every bit outside [`TerminalFlags`] is undefined, and is reported as such.
fn decode_flags(word: u32) -> Result<TerminalFlags, Errno> {
    TerminalFlags::from_bits(word).ok_or_else(|| {
        crate::unsupported::unsupported_argument(
            "ioctl.tcsetattr.flags",
            u64::from(word),
            Errno::NotSupported,
        )
    })
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

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;
    use roxy_tty_types::TerminalFlags;

    use super::decode_flags;

    kernel_test!(
        "roxy-syscall::terminal-attribute-flags",
        decodes_defined_bits,
        {
            assert_eq!(
                decode_flags(TerminalFlags::ECHO.bits()),
                Ok(TerminalFlags::ECHO)
            );
            assert_eq!(
                decode_flags((TerminalFlags::ISIG | TerminalFlags::OPOST).bits()),
                Ok(TerminalFlags::ISIG | TerminalFlags::OPOST)
            );
        }
    );

    kernel_test!(
        "roxy-syscall::terminal-attribute-flags",
        rejects_undefined_bits,
        {
            assert!(decode_flags(1 << 8).is_err());
            assert!(decode_flags(u32::MAX).is_err());
        }
    );
}
