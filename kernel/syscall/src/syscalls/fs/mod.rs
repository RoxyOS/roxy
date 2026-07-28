mod dir;
mod link;
mod misc;

use roxy_fd::FileError;
use roxy_vfs::VfsError;

use crate::{Syscall, args::SyscallArg, errno::Errno};

pub(super) const MKDIRAT_SYSCALL: Syscall = dir::MKDIRAT_SYSCALL;
pub(super) const UNLINKAT_SYSCALL: Syscall = dir::UNLINKAT_SYSCALL;
pub(super) const READLINKAT_SYSCALL: Syscall = link::READLINKAT_SYSCALL;
pub(super) const LINKAT_SYSCALL: Syscall = link::LINKAT_SYSCALL;
pub(super) const SYMLINKAT_SYSCALL: Syscall = link::SYMLINKAT_SYSCALL;
pub(super) const RENAMEAT_SYSCALL: Syscall = link::RENAMEAT_SYSCALL;
pub(super) const SYNC_SYSCALL: Syscall = misc::SYNC_SYSCALL;
pub(super) const FSYNC_SYSCALL: Syscall = misc::FSYNC_SYSCALL;

const AT_FDCWD: i64 = -100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectoryFd(i64);

impl DirectoryFd {
    fn require_cwd(self, operation: &str) -> Result<(), Errno> {
        if self.0 != AT_FDCWD {
            return Err(unsupported(operation, self.0));
        }

        Ok(())
    }
}

impl SyscallArg for DirectoryFd {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        Ok(Self(raw.cast_signed()))
    }
}

fn map_file_error(error: FileError) -> Errno {
    match error {
        FileError::BadOperation => unsupported("fsync.fd-object", 0),
        FileError::Io => Errno::Io,
    }
}

fn map_vfs_error(error: VfsError) -> Errno {
    match error {
        VfsError::NotInitialized | VfsError::Io | VfsError::Corrupt => Errno::Io,
        VfsError::InvalidPath | VfsError::InvalidInput => Errno::Invalid,
        VfsError::DirectoryNotEmpty => Errno::NotEmpty,
        VfsError::NotFound => Errno::NotFound,
        VfsError::AlreadyExists => Errno::AlreadyExists,
        VfsError::NotDirectory => Errno::NotDirectory,
        VfsError::IsDirectory => Errno::IsDirectory,
        VfsError::ReadOnly => Errno::ReadOnly,
        VfsError::PermissionDenied => Errno::Access,
        VfsError::NoSpace => Errno::NoSpace,
        VfsError::Busy => Errno::Busy,
        VfsError::CrossDevice => Errno::CrossDevice,
        VfsError::Unsupported => unsupported("fs.filesystem", 0),
    }
}

fn unsupported(operation: &str, argument: impl core::fmt::Display) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
