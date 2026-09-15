use alloc::boxed::Box;

use roxy_fd::OpenFile;
use roxy_vfs::VfsError;

use crate::{SyscallResult, args::CString, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::OpenDir, handle(path: CString => Fault));

fn handle(path: CString) -> SyscallResult {
    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    let directory = roxy_vfs::open_dir(path.into_inner()).map_err(map_vfs_error)?;
    let file = OpenFile::new(Box::new(directory));
    let fd = roxy_process::insert_open_file(file, false);

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
        VfsError::Loop => Errno::Loop,
        VfsError::Unsupported => {
            crate::unsupported::unsupported_argument("open_dir.filesystem", 0, Errno::NotSupported)
        }
        VfsError::WouldBlock => Errno::Again,
    }
}
