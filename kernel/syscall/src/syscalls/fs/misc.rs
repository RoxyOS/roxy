use roxy_fd::Fd;

use super::{map_file_error, map_vfs_error};
use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

pub(super) const SYNC_SYSCALL: Syscall = sync::SYSCALL;
pub(super) const FSYNC_SYSCALL: Syscall = fsync::SYSCALL;

mod sync {
    use super::{SyscallNumber, SyscallResult, map_vfs_error, syscall};

    syscall!(SyscallNumber::Sync, handle());

    fn handle() -> SyscallResult {
        roxy_vfs::sync().map_err(map_vfs_error)?;

        Ok(0)
    }
}

mod fsync {
    use super::{Errno, Fd, SyscallNumber, SyscallResult, map_file_error, syscall};

    syscall!(SyscallNumber::Fsync, handle(fd: Fd => BadFd));

    fn handle(fd: Fd) -> SyscallResult {
        let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;

        file.sync().map_err(map_file_error)?;

        Ok(0)
    }
}
