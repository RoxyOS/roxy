use roxy_vfs::FilePermissions;

use crate::{
    SyscallResult,
    args::CString,
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::Chmod, handle(
    path: CString => Fault,
    permissions: FilePermissions => Invalid,
));

fn handle(path: CString, permissions: FilePermissions) -> SyscallResult {
    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    roxy_vfs::chmod(path.into_inner(), permissions).map_err(map_vfs_error)?;

    Ok(0)
}

fn map_vfs_error(error: roxy_vfs::VfsError) -> Errno {
    match error {
        roxy_vfs::VfsError::NotFound => Errno::NotFound,
        roxy_vfs::VfsError::PermissionDenied => Errno::Access,
        roxy_vfs::VfsError::ReadOnly => Errno::ReadOnly,
        roxy_vfs::VfsError::Unsupported | roxy_vfs::VfsError::InvalidInput => Errno::Invalid,
        _ => Errno::Io,
    }
}
