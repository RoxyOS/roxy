use roxy_fd::Fd;
use roxy_vfs::FilePermissions;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Fchmod, handle(
    fd: Fd => BadFd,
    permissions: FilePermissions => Invalid,
));

fn handle(fd: Fd, permissions: FilePermissions) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;

    file.set_permissions(permissions.bits())
        .map_err(|_| Errno::Io)?;

    Ok(0)
}
