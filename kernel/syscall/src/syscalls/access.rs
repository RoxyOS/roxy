use alloc::vec::Vec;
use bitflags::bitflags;
use roxy_vfs::AccessMode;

use crate::{
    SyscallResult,
    args::{CString, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

/// Lowest bit of the Roxy `access` mode word, which every request carries.
///
/// Linux's modes are `F_OK` 0, `X_OK` 1, `W_OK` 2, and `R_OK` 4, so a word below this base is
/// another personality's numbering, and the handler reports a caller that passes one as foreign
/// instead of reading it as a request of its own.
const ACCESS_BASE: u64 = 0x100;

/// The base is above every Linux access mode, so a mode below it is never ours.
const _: () = assert!(ACCESS_BASE > 0b111);

bitflags! {
    /// The `access` mode word, one bit per right.
    ///
    /// Each flag is its own bit rather than a base shared by all of them, so a caller's `&` test
    /// for one right cannot answer true because another right was requested. `EXISTS` is the base
    /// and the only flag `F_OK` sets. Values match `abi-bits/access.h`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct AccessFlags: u64 {
        const EXISTS = ACCESS_BASE;
        const EXECUTE = ACCESS_BASE << 1;
        const WRITE = ACCESS_BASE << 2;
        const READ = ACCESS_BASE << 3;
    }
}

impl SyscallArg for AccessFlags {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        let unknown = raw & !Self::all().bits();

        if unknown != 0 {
            return Err(unsupported("access.mode", unknown, Errno::Invalid));
        }

        if raw < ACCESS_BASE {
            return Err(unsupported("access.mode.foreign", raw, Errno::Invalid));
        }

        Ok(Self::from_bits_retain(raw))
    }
}

syscall!(SyscallNumber::Access, handle(
    path: CString => Fault,
    mode: AccessFlags => Invalid,
));

fn handle(path: CString, mode: AccessFlags) -> SyscallResult {
    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    roxy_vfs::access(path.into_inner(), &rights(mode)).map_err(map_vfs_error)?;

    Ok(0)
}

/// Decodes the request's permission bits into the ABI-neutral rights the VFS checks.
///
/// `EXISTS` asks for nothing beyond the existence the metadata lookup already establishes, so it
/// decodes to no right at all, which is what `F_OK` means. An empty result is the existence test.
fn rights(mode: AccessFlags) -> Vec<AccessMode> {
    let mut rights = Vec::new();

    if mode.contains(AccessFlags::READ) {
        rights.push(AccessMode::Read);
    }
    if mode.contains(AccessFlags::WRITE) {
        rights.push(AccessMode::Write);
    }
    if mode.contains(AccessFlags::EXECUTE) {
        rights.push(AccessMode::Execute);
    }

    rights
}

fn map_vfs_error(error: roxy_vfs::VfsError) -> Errno {
    match error {
        roxy_vfs::VfsError::NotFound | roxy_vfs::VfsError::InvalidPath => Errno::NotFound,
        roxy_vfs::VfsError::PermissionDenied => Errno::Access,
        _ => Errno::Io,
    }
}

fn unsupported(operation: &str, argument: u64, errno: Errno) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, errno)
}
