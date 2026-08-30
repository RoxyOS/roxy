use alloc::vec::Vec;
use core::mem::{offset_of, size_of};

use bitflags::bitflags;
use roxy_fd::FileError;
use roxy_memory::UserAddress;

use crate::{
    args::{SyscallArg, user_memory},
    errno::Errno,
};

// ── ABI layout ─────────────────────────────────────────────────────────────

/// ABI-compatible `struct msghdr` for `x86_64` little-endian.
///
/// Pointer fields (`msg_name`, `msg_iov`, `msg_control`) are deserialized as
/// `UserAddress` so the handler can read or write through them directly.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MsgHdr {
    pub(crate) msg_name: UserAddress,
    pub(crate) msg_namelen: u32,
    _pad0: u32,
    pub(crate) msg_iov: UserAddress,
    pub(crate) msg_iovlen: i32,
    _pad1: u32,
    pub(crate) msg_control: UserAddress,
    pub(crate) msg_controllen: u32,
    _pad2: u32,
    pub(crate) msg_flags: i32,
}

const _: () = assert!(size_of::<MsgHdr>() == 56);
const _: () = assert!(offset_of!(MsgHdr, msg_controllen) == 40);
const _: () = assert!(offset_of!(MsgHdr, msg_flags) == 48);

/// A parsed `msghdr` together with the user-space address of the record itself, so the handler
/// can write back its mutable fields (`msg_namelen`, `msg_controllen`, `msg_flags`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ParsedMsgHdr {
    header: MsgHdr,
    source: UserAddress,
}

impl ParsedMsgHdr {
    #[inline]
    #[must_use]
    pub(crate) const fn msg_iov(self) -> UserAddress {
        self.header.msg_iov
    }

    #[inline]
    #[must_use]
    pub(crate) const fn msg_iovlen(self) -> i32 {
        self.header.msg_iovlen
    }

    #[inline]
    #[must_use]
    pub(crate) const fn msg_control(self) -> UserAddress {
        self.header.msg_control
    }

    #[inline]
    #[must_use]
    pub(crate) const fn msg_controllen(self) -> u32 {
        self.header.msg_controllen
    }

    /// Writes back the `msg_controllen` field of the user-space record.
    pub(crate) fn write_controllen(&self, value: u32) -> Result<(), Errno> {
        let address = self
            .source
            .checked_add(offset_of!(MsgHdr, msg_controllen) as u64)
            .ok_or(Errno::Fault)?;
        // SAFETY: u32 is stable, no padding, every bit pattern valid.
        unsafe { user_memory::write(address, &value) }
    }

    /// Writes back the `msg_flags` field of the user-space record.
    pub(crate) fn write_flags(&self, value: i32) -> Result<(), Errno> {
        let address = self
            .source
            .checked_add(offset_of!(MsgHdr, msg_flags) as u64)
            .ok_or(Errno::Fault)?;
        // SAFETY: i32 is stable, no padding, every bit pattern valid.
        unsafe { user_memory::write(address, &value) }
    }
}

/// ABI-compatible `struct iovec` for `x86_64`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Iovec {
    pub(crate) base: UserAddress,
    pub(crate) length: usize,
}

const _: () = assert!(size_of::<Iovec>() == 16);

// ── Flags ──────────────────────────────────────────────────────────────────

bitflags! {
    /// Flags recognised by `recvmsg`/`sendmsg` (`MSG_*` constants).
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct MsgFlags: u32 {
        const DONTWAIT = 0x40;
        const NOSIGNAL = 0x4000;
    }
}

impl SyscallArg for MsgFlags {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        // The `int flags` argument is sign-extended in the register; truncate to the lower 32
        // bits to obtain the true value.
        #[allow(clippy::cast_possible_truncation)]
        let raw = raw as u32;
        let unknown = raw & !Self::all().bits();

        if unknown != 0 {
            return Err(unsupported("msg.flags", u64::from(unknown)));
        }

        Ok(Self::from_bits_retain(raw))
    }
}

// ── SyscallArg ─────────────────────────────────────────────────────────────

impl SyscallArg for ParsedMsgHdr {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let source = UserAddress::parse(raw, error)?;
        let mut header = MsgHdr {
            msg_name: UserAddress::sentinel(),
            msg_namelen: 0,
            _pad0: 0,
            msg_iov: UserAddress::sentinel(),
            msg_iovlen: 0,
            _pad1: 0,
            msg_control: UserAddress::sentinel(),
            msg_controllen: 0,
            _pad2: 0,
            msg_flags: 0,
        };

        // SAFETY: MsgHdr is repr(C) with explicit padding (no implicit gaps), every field is an
        // integer or a UserAddress newtype, so every bit pattern from userspace is valid.
        unsafe { user_memory::read(source, &mut header) }?;

        Ok(Self { header, source })
    }
}

// ── I/O helpers ────────────────────────────────────────────────────────────

/// Maximum bytes to transfer in one gather/scatter pass.
const SCRATCH_SIZE: usize = 4096;
/// Upper bound for a single recvmsg scatter buffer.
const MAX_RECV_BUFFER: usize = SCRATCH_SIZE * 16;

/// Reads the `iovec` array at `address` (count = `count`) from user space.
///
/// Returns an empty vector when `count` is zero or negative.
///
/// # Errors
///
/// Returns `Fault` when the iovec array cannot be read.
pub(crate) fn read_iovecs(address: UserAddress, count: i32) -> Result<Vec<Iovec>, Errno> {
    if count <= 0 {
        return Ok(Vec::new());
    }

    #[allow(clippy::cast_sign_loss)]
    let count = count as usize;
    let mut iovecs = Vec::<Iovec>::new();
    iovecs.try_reserve_exact(count).map_err(|_| Errno::NoMem)?;
    iovecs.resize(
        count,
        Iovec {
            base: UserAddress::sentinel(),
            length: 0,
        },
    );

    // SAFETY: Iovec is repr(C) with integer/pointer fields that accept every bit pattern.
    unsafe { user_memory::read_slice(address, &mut iovecs) }?;

    Ok(iovecs)
}

/// Gathers data from user-space iovecs into the file, honoring `nonblocking`.
///
/// Returns the total number of bytes written.
///
/// # Errors
///
/// Returns `FileError::Io` when a user buffer cannot be read, plus any error from the write.
pub(crate) fn sendmsg_gather(
    file: &roxy_fd::OpenFile,
    iovecs: &[Iovec],
    nonblocking: bool,
) -> Result<usize, FileError> {
    let mut written = 0usize;

    for iov in iovecs {
        let mut remaining = iov.length;

        while remaining > 0 {
            let chunk = remaining.min(SCRATCH_SIZE);
            let mut buf = [0u8; SCRATCH_SIZE];

            // SAFETY: u8 accepts every byte pattern; the slice is bounded by the iovec length.
            unsafe { user_memory::read_slice(iov.base, &mut buf[..chunk]) }
                .map_err(|_| FileError::Io)?;

            let n = file.write_with_nonblocking(&buf[..chunk], nonblocking)?;
            written += n;

            if n < chunk {
                // Partial write: the file (or nonblocking limit) cannot accept more right now.
                return Ok(written);
            }

            remaining -= chunk;
        }
    }

    Ok(written)
}

/// Reads through the file (honoring `nonblocking`), then scatters the bytes into user iovecs.
///
/// Returns the total number of bytes read.
///
/// # Errors
///
/// Returns `FileError::Io` when a user buffer cannot be written, plus any error from the read.
pub(crate) fn recvmsg_scatter(
    file: &roxy_fd::OpenFile,
    iovecs: &[Iovec],
    nonblocking: bool,
) -> Result<usize, FileError> {
    if iovecs.is_empty() {
        return Ok(0);
    }

    let total_capacity: usize = iovecs.iter().map(|iov| iov.length).sum();

    if total_capacity == 0 {
        return Ok(0);
    }

    // Read into a local contiguous buffer (bounded), then scatter across iovecs.
    let capacity = total_capacity.min(MAX_RECV_BUFFER);
    let mut buf = Vec::<u8>::new();
    buf.try_reserve_exact(capacity).map_err(|_| FileError::Io)?;
    buf.resize(capacity, 0);

    let read = file.read_with_nonblocking(&mut buf, nonblocking)?;

    if read == 0 {
        return Ok(0);
    }

    let mut offset = 0usize;
    for iov in iovecs {
        if offset >= read {
            break;
        }

        let remaining = read - offset;
        let chunk = remaining.min(iov.length);

        if chunk == 0 {
            continue;
        }

        // SAFETY: buf[offset..offset+chunk] is initialized from the read.
        unsafe { user_memory::write_slice(iov.base, &buf[offset..offset + chunk]) }
            .map_err(|_| FileError::Io)?;

        offset += chunk;
    }

    Ok(read)
}

// ── Diagnostics ────────────────────────────────────────────────────────────

fn unsupported(operation: &str, argument: impl core::fmt::Display) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
