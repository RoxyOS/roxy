//! Ioctl dispatch for evdev (EVIOC*) requests.
//!
//! The main entry point is [`execute`], which is called from the parent ioctl handler after the
//! request type byte has been confirmed as `'E'` (0x45). It dispatches by the `nr` byte.

use roxy_fd::{IoctlRequest, OpenFile};
use roxy_memory::UserAddress;

use crate::{
    args::{SyscallArg, user_memory},
    errno::Errno,
};

use super::evdev_abi;

pub(super) use evdev_abi::is_evioc_request;

// ── Dispatch ────────────────────────────────────────────────────────────────

pub(super) fn execute(file: &OpenFile, raw_request: u64, raw_argument: u64) -> Result<u64, Errno> {
    let nr = evdev_abi::evioc_nr(raw_request);
    let size = evdev_abi::evioc_size(raw_request);
    let address = UserAddress::parse(raw_argument, Errno::Fault)?;

    match nr {
        0x01 => get_version(file, address),
        0x02 => get_id(file, address),
        0x03 => {
            if evdev_abi::is_read_direction(raw_request) {
                get_rep(file, address)
            } else {
                set_rep(file, address)
            }
        }
        0x06 => get_name(file, address, size),
        0x07 => get_phys(file, address, size),
        0x08 => get_uniq(file, address, size),
        0x09 => get_prop(file, address, size),
        0x18 => get_key_state(file, address, size),
        0x19 => get_led_state(file, address, size),
        0x1a => get_sw_state(file, address, size),
        0x20..=0x3f => get_bits(file, address, size, u16::from(nr - 0x20)),
        0x40..=0x7f => get_abs(file, address, u16::from(nr - 0x40)),
        0x90 => grab(file, address),
        0xa0 => set_clock_id(file, address),
        0xc0..=0xff => set_abs(file, address, u16::from(nr - 0xc0)),
        _ => Err(Errno::NotTty),
    }
}

// ── Fixed-size output handlers ──────────────────────────────────────────────

fn get_version(file: &OpenFile, address: UserAddress) -> Result<u64, Errno> {
    let mut version = 0u32;
    file.ioctl(IoctlRequest::EvdevGetVersion(&mut version))
        .map_err(map_ioctl_error)?;
    // SAFETY: u32 has no padding and every bit pattern is valid.
    unsafe { user_memory::write(address, &version) }?;
    Ok(0)
}

fn get_id(file: &OpenFile, address: UserAddress) -> Result<u64, Errno> {
    let mut id = roxy_evdev_types::EvdevDeviceId::default();
    file.ioctl(IoctlRequest::EvdevGetId(&mut id))
        .map_err(map_ioctl_error)?;
    let abi = evdev_abi::InputIdAbi {
        bustype: id.bustype,
        vendor: id.vendor,
        product: id.product,
        version: id.version,
    };
    // SAFETY: InputIdAbi is repr(C) with no implicit padding and all fields initialized.
    unsafe { user_memory::write(address, &abi) }?;
    Ok(0)
}

fn get_rep(file: &OpenFile, address: UserAddress) -> Result<u64, Errno> {
    let mut rep = [0u8; 8];
    file.ioctl(IoctlRequest::EvdevGetRep(&mut rep))
        .map_err(map_ioctl_error)?;
    // SAFETY: [u8; 8] has no padding and every bit pattern is valid.
    unsafe { user_memory::write_slice(address, &rep) }?;
    Ok(0)
}

fn set_rep(file: &OpenFile, address: UserAddress) -> Result<u64, Errno> {
    let mut rep = [0u8; 8];
    // SAFETY: [u8; 8] has no padding and every bit pattern is valid.
    unsafe { user_memory::read_slice(address, &mut rep) }?;
    file.ioctl(IoctlRequest::EvdevSetRep(&rep))
        .map_err(map_ioctl_error)
        .map(|()| 0)
}

fn grab(file: &OpenFile, address: UserAddress) -> Result<u64, Errno> {
    let mut value = 0i32;
    // SAFETY: i32 has no padding.
    unsafe { user_memory::read(address, &mut value) }?;
    file.ioctl(IoctlRequest::EvdevGrab(value != 0))
        .map_err(map_ioctl_error)
        .map(|()| 0)
}

fn set_clock_id(file: &OpenFile, address: UserAddress) -> Result<u64, Errno> {
    let mut clock_id = 0i32;
    // SAFETY: i32 has no padding.
    unsafe { user_memory::read(address, &mut clock_id) }?;
    file.ioctl(IoctlRequest::EvdevSetClockId(clock_id))
        .map_err(map_ioctl_error)
        .map(|()| 0)
}

// ── Variable-size string handlers ───────────────────────────────────────────
//
// For EVIOCGNAME/GPHYS/GUNIQ the device writes a null-terminated string into the slice. The
// syscall returns `strlen` computed by scanning for the first NUL.

fn get_name(file: &OpenFile, address: UserAddress, size: usize) -> Result<u64, Errno> {
    let mut buffer = alloc::vec![0u8; size];
    file.ioctl(IoctlRequest::EvdevGetName(&mut buffer[..]))
        .map_err(map_ioctl_error)?;
    // SAFETY: buffer is initialized and contains only u8.
    unsafe { user_memory::write_slice(address, &buffer) }?;
    let len = buffer.iter().position(|&b| b == 0).unwrap_or(size);
    Ok(len as u64)
}

fn get_phys(file: &OpenFile, address: UserAddress, size: usize) -> Result<u64, Errno> {
    let mut buffer = alloc::vec![0u8; size];
    file.ioctl(IoctlRequest::EvdevGetPhys(&mut buffer[..]))
        .map_err(map_ioctl_error)?;
    // SAFETY: buffer is initialized.
    unsafe { user_memory::write_slice(address, &buffer) }?;
    let len = buffer.iter().position(|&b| b == 0).unwrap_or(size);
    Ok(len as u64)
}

fn get_uniq(file: &OpenFile, address: UserAddress, size: usize) -> Result<u64, Errno> {
    let mut buffer = alloc::vec![0u8; size];
    file.ioctl(IoctlRequest::EvdevGetUniq(&mut buffer[..]))
        .map_err(map_ioctl_error)?;
    // SAFETY: buffer is initialized.
    unsafe { user_memory::write_slice(address, &buffer) }?;
    let len = buffer.iter().position(|&b| b == 0).unwrap_or(size);
    Ok(len as u64)
}

// ── Static capability handlers (computed from roxy-evdev-types tables) ──────
//
// These ioctls return bitmaps that describe the device's capabilities. They are
// computed directly, without a round-trip to the device, because the capability set
// is a static property of the keyboard evdev device.

fn get_prop(_file: &OpenFile, address: UserAddress, size: usize) -> Result<u64, Errno> {
    // Keyboard has no special device properties.
    let len = size.min(1);
    let zeroes = alloc::vec![0u8; len];
    // SAFETY: zeroes is initialized.
    unsafe { user_memory::write_slice(address, &zeroes) }?;
    Ok(len as u64)
}

fn get_key_state(_file: &OpenFile, address: UserAddress, size: usize) -> Result<u64, Errno> {
    // Not tracked yet → all zeros (no keys pressed).
    let zeroes = alloc::vec![0u8; size];
    // SAFETY: zeroes is initialized.
    unsafe { user_memory::write_slice(address, &zeroes) }?;
    Ok(size as u64)
}

fn get_led_state(_file: &OpenFile, address: UserAddress, size: usize) -> Result<u64, Errno> {
    // Not tracked yet → all zeros (no LEDs on).
    let zeroes = alloc::vec![0u8; size];
    // SAFETY: zeroes is initialized.
    unsafe { user_memory::write_slice(address, &zeroes) }?;
    Ok(size as u64)
}

fn get_sw_state(_file: &OpenFile, address: UserAddress, size: usize) -> Result<u64, Errno> {
    // No switches on a keyboard → all zero.
    let zeroes = alloc::vec![0u8; size];
    // SAFETY: zeroes is initialized.
    unsafe { user_memory::write_slice(address, &zeroes) }?;
    Ok(size as u64)
}

fn get_bits(file: &OpenFile, address: UserAddress, size: usize, ev: u16) -> Result<u64, Errno> {
    let mut buffer = alloc::vec![0u8; size];
    let mut written = 0usize;
    file.ioctl(IoctlRequest::EvdevGetBits {
        ev,
        buffer: &mut buffer,
        written: &mut written,
    })
    .map_err(map_ioctl_error)?;
    // SAFETY: buffer is initialized.
    unsafe { user_memory::write_slice(address, &buffer) }?;
    Ok(written.min(size) as u64)
}

fn get_abs(_file: &OpenFile, address: UserAddress, _axis: u16) -> Result<u64, Errno> {
    // Keyboard has no absolute axes → return all zeros.
    let abi = evdev_abi::InputAbsInfoAbi {
        value: 0,
        minimum: 0,
        maximum: 0,
        fuzz: 0,
        flat: 0,
        resolution: 0,
    };
    // SAFETY: InputAbsInfoAbi is repr(C) and all fields initialized.
    unsafe { user_memory::write(address, &abi) }?;
    Ok(0)
}

fn set_abs(_file: &OpenFile, _address: UserAddress, _axis: u16) -> Result<u64, Errno> {
    // Keyboard has no absolute axes; reject with ENOTSUP.
    // TODO(evdev-abs): support absolute axes when a pointer device is added.
    Err(Errno::NotSupported)
}

fn map_ioctl_error(error: roxy_fd::IoctlError) -> Errno {
    match error {
        roxy_fd::IoctlError::NotTty => Errno::NotTty,
        roxy_fd::IoctlError::Invalid => Errno::Invalid,
        roxy_fd::IoctlError::Unsupported {
            operation,
            argument,
        } => crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported),
    }
}
