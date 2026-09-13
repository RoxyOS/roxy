use bitflags::bitflags;
use roxy_vfs::FilePermissions;

use super::{DirectoryFd, map_vfs_error};
use crate::{
    Syscall, SyscallResult,
    args::{Path, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

pub(super) const MKDIRAT_SYSCALL: Syscall = mkdirat::SYSCALL;
pub(super) const UNLINKAT_SYSCALL: Syscall = unlinkat::SYSCALL;

/// Roxy numbers `AT_*` flags from a base above Linux's whole range for the word, whose top is
/// `AT_RECURSIVE` at bit 15, so no Linux flag can alias one of ours: a value below the base is
/// another personality's numbering, which the handler reports as foreign.
const AT_FLAGS_BASE: u64 = 1 << 16;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct UnlinkFlags: u64 {
        const REMOVE_DIR = AT_FLAGS_BASE << 1;
    }
}

impl SyscallArg for UnlinkFlags {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        // Anything below the Roxy base is another personality's numbering.
        let foreign = raw & (AT_FLAGS_BASE - 1);

        if foreign != 0 {
            return Err(crate::unsupported::unsupported_argument(
                "unlinkat.flags.foreign",
                foreign,
                Errno::Invalid,
            ));
        }

        let unknown = raw & !Self::all().bits();

        if unknown != 0 {
            return Err(crate::unsupported::unsupported_argument(
                "unlinkat.flags.unknown",
                unknown,
                Errno::Invalid,
            ));
        }

        Ok(Self::from_bits_retain(raw))
    }
}

mod mkdirat {
    use super::{
        DirectoryFd, FilePermissions, Path, SyscallNumber, SyscallResult, map_vfs_error, syscall,
    };

    syscall!(SyscallNumber::Mkdirat, handle(dirfd: DirectoryFd => Invalid, path: Path => Fault, permissions: FilePermissions => Invalid));

    fn handle(dirfd: DirectoryFd, path: Path, permissions: FilePermissions) -> SyscallResult {
        dirfd.require_cwd("mkdirat.dirfd", "mkdirat.dirfd.foreign")?;

        roxy_vfs::mkdir(path.into_inner(), permissions).map_err(map_vfs_error)?;

        Ok(0)
    }
}

mod unlinkat {
    use super::{
        DirectoryFd, Path, SyscallNumber, SyscallResult, UnlinkFlags, map_vfs_error, syscall,
    };

    syscall!(SyscallNumber::Unlinkat, handle(dirfd: DirectoryFd => Invalid, path: Path => Fault, flags: UnlinkFlags => Invalid));

    fn handle(dirfd: DirectoryFd, path: Path, flags: UnlinkFlags) -> SyscallResult {
        dirfd.require_cwd("unlinkat.dirfd", "unlinkat.dirfd.foreign")?;

        if flags.contains(UnlinkFlags::REMOVE_DIR) {
            roxy_vfs::rmdir(path.into_inner()).map_err(map_vfs_error)?;
        } else {
            roxy_vfs::unlink(path.into_inner()).map_err(map_vfs_error)?;
        }

        Ok(0)
    }
}
