use roxy_vfs::{FileType, ResolvedPath, VfsError};

use crate::{SyscallResult, args::CString, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Chdir, handle(path: CString => Fault));

fn handle(path: CString) -> SyscallResult {
    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    let path = ResolvedPath::resolve(path.into_inner()).map_err(map_vfs_error)?;
    let metadata = roxy_vfs::metadata(path.as_bytes()).map_err(map_vfs_error)?;

    if metadata.file_type != FileType::Directory {
        return Err(Errno::NotDirectory);
    }

    roxy_process::set_current_working_directory(path);

    Ok(0)
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
            crate::unsupported::unsupported_argument("chdir.filesystem", 0, Errno::NotSupported)
        }
    }
}
