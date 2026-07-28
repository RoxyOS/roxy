use roxy_memory::UserAddress;

use super::{DirectoryFd, map_vfs_error, unsupported};
use crate::{
    Syscall, SyscallResult,
    args::{CString, Path, Slice},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

pub(super) const READLINKAT_SYSCALL: Syscall = readlinkat::SYSCALL;
pub(super) const LINKAT_SYSCALL: Syscall = linkat::SYSCALL;
pub(super) const SYMLINKAT_SYSCALL: Syscall = symlinkat::SYSCALL;
pub(super) const RENAMEAT_SYSCALL: Syscall = renameat::SYSCALL;

mod readlinkat {
    use super::*;

    syscall!(SyscallNumber::Readlinkat, handle(dirfd: DirectoryFd => Invalid, path: Path => Fault, buffer: UserAddress => Fault, size: usize => Fault));

    fn handle(dirfd: DirectoryFd, path: Path, buffer: UserAddress, size: usize) -> SyscallResult {
        dirfd.require_cwd("readlinkat.dirfd")?;

        if size == 0 {
            return Err(Errno::Invalid);
        }

        let target = roxy_vfs::read_link(path.into_inner()).map_err(map_vfs_error)?;
        let written = target.len().min(size);

        if written != 0 {
            let output = Slice::<u8>::new(buffer, written);
            // SAFETY: u8 has no padding and every byte in target is initialized.
            unsafe { output.write(&target[..written]) }?;
        }

        Ok(u64::try_from(written).unwrap())
    }
}

mod linkat {
    use super::*;

    syscall!(SyscallNumber::Linkat, handle(old_dirfd: DirectoryFd => Invalid, old_path: Path => Fault, new_dirfd: DirectoryFd => Invalid, new_path: Path => Fault, flags: u64));

    fn handle(
        old_dirfd: DirectoryFd,
        old_path: Path,
        new_dirfd: DirectoryFd,
        new_path: Path,
        flags: u64,
    ) -> SyscallResult {
        if flags != 0 {
            return Err(unsupported("linkat.flags", flags));
        }
        old_dirfd.require_cwd("linkat.old_dirfd")?;
        new_dirfd.require_cwd("linkat.new_dirfd")?;

        roxy_vfs::hard_link(old_path.into_inner(), new_path.into_inner()).map_err(map_vfs_error)?;

        Ok(0)
    }
}

mod symlinkat {
    use super::*;

    syscall!(SyscallNumber::Symlinkat, handle(target: CString => Fault, dirfd: DirectoryFd => Invalid, link: Path => Fault));

    fn handle(target: CString, dirfd: DirectoryFd, link: Path) -> SyscallResult {
        dirfd.require_cwd("symlinkat.dirfd")?;

        roxy_vfs::symlink(target.into_inner(), link.into_inner()).map_err(map_vfs_error)?;

        Ok(0)
    }
}

mod renameat {
    use super::*;

    syscall!(SyscallNumber::Renameat, handle(old_dirfd: DirectoryFd => Invalid, old_path: Path => Fault, new_dirfd: DirectoryFd => Invalid, new_path: Path => Fault));

    fn handle(
        old_dirfd: DirectoryFd,
        old_path: Path,
        new_dirfd: DirectoryFd,
        new_path: Path,
    ) -> SyscallResult {
        old_dirfd.require_cwd("renameat.old_dirfd")?;
        new_dirfd.require_cwd("renameat.new_dirfd")?;

        roxy_vfs::rename(old_path.into_inner(), new_path.into_inner()).map_err(map_vfs_error)?;

        Ok(0)
    }
}
