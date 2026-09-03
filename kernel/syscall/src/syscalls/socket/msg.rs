use core::mem::{offset_of, size_of};

use bitflags::bitflags;
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

fn unsupported(operation: &str, argument: impl core::fmt::Display) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
