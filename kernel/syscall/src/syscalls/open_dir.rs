use alloc::boxed::Box;

use roxy_fd::OpenFile;
use roxy_memory::UserAddress;
use roxy_vfs::{ResolvedPath, VfsError};

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::OpenDir, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let path_address = UserAddress::new(arguments[0]).ok_or(Errno::Fault)?;

    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;
    let path = crate::user::read_c_string(&addrspace, path_address, ResolvedPath::MAX_LEN)?;

    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    let directory = roxy_vfs::open_dir(path).map_err(map_vfs_error)?;
    let file = OpenFile::new(Box::new(directory));
    let fd = roxy_process::insert_open_file(file);

    Ok(u64::from(fd.as_u32()))
}

fn map_vfs_error(error: VfsError) -> Errno {
    match error {
        VfsError::NotInitialized | VfsError::Io | VfsError::Corrupt => Errno::Io,
        VfsError::InvalidPath | VfsError::InvalidInput | VfsError::DirectoryNotEmpty => {
            Errno::Invalid
        }
        VfsError::NotFound => Errno::NotFound,
        VfsError::AlreadyExists => Errno::AlreadyExists,
        VfsError::NotDirectory | VfsError::IsDirectory => Errno::NotDirectory,
        VfsError::ReadOnly => Errno::ReadOnly,
        VfsError::PermissionDenied => Errno::Access,
        VfsError::NoSpace => Errno::NoSpace,
        VfsError::Busy => Errno::Busy,
        VfsError::CrossDevice => Errno::CrossDevice,
        VfsError::Unsupported => {
            crate::unsupported::unsupported_argument("open_dir.filesystem", 0, Errno::NotSupported)
        }
    }
}
