use roxy_vfs::FilePermissions;

use super::{AtFlags, DirectoryFd, map_vfs_error};
use crate::{Syscall, SyscallResult, args::Path, numbers::SyscallNumber, syscall};

pub(super) const MKDIRAT_SYSCALL: Syscall = mkdirat::SYSCALL;
pub(super) const UNLINKAT_SYSCALL: Syscall = unlinkat::SYSCALL;

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
    use super::{AtFlags, DirectoryFd, Path, SyscallNumber, SyscallResult, map_vfs_error, syscall};

    syscall!(SyscallNumber::Unlinkat, handle(dirfd: DirectoryFd => Invalid, path: Path => Fault, flags: AtFlags => Invalid));

    fn handle(dirfd: DirectoryFd, path: Path, flags: AtFlags) -> SyscallResult {
        flags.require_only(AtFlags::REMOVE_DIR, "unlinkat.flags")?;
        dirfd.require_cwd("unlinkat.dirfd", "unlinkat.dirfd.foreign")?;

        if flags.contains(AtFlags::REMOVE_DIR) {
            roxy_vfs::rmdir(path.into_inner()).map_err(map_vfs_error)?;
        } else {
            roxy_vfs::unlink(path.into_inner()).map_err(map_vfs_error)?;
        }

        Ok(0)
    }
}
