mod dir;
mod link;
mod misc;
mod truncate;

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
pub(super) const FTRUNCATE_SYSCALL: Syscall = truncate::SYSCALL;

/// Roxy numbers `dirfd` selectors from a base above Linux's range, so a Linux-valued selector —
/// including its `AT_FDCWD` of -100 — is reported as a foreign numbering.
const AT_BASE: i64 = 1 << 8;
const AT_FDCWD: i64 = AT_BASE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectoryFd(i64);

impl DirectoryFd {
    /// Accepts only the working-directory selector, reporting the caller's value through the
    /// centralized diagnostic otherwise. `operation` names the argument for a descriptor Roxy
    /// cannot serve yet and `foreign` for a negative selector, which is another personality's
    /// numbering: Linux spells the working directory as -100.
    fn require_cwd(self, operation: &str, foreign: &str) -> Result<(), Errno> {
        if self.0 == AT_FDCWD {
            return Ok(());
        }

        Err(unsupported(
            if self.0 < 0 { foreign } else { operation },
            self.0,
        ))
    }
}

impl SyscallArg for DirectoryFd {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        Ok(Self(raw.cast_signed()))
    }
}

fn map_file_error(error: FileError) -> Errno {
    match error {
        FileError::WouldBlock => Errno::Again,
        FileError::BadOperation => unsupported("fsync.fd-object", 0),
        FileError::BrokenPipe => Errno::Pipe,
        FileError::NotConnected => Errno::NotConnected,
        FileError::Io => Errno::Io,
        FileError::Interrupted => Errno::Interrupted,
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
        VfsError::Loop => Errno::Loop,
        VfsError::Unsupported => unsupported("fs.filesystem", 0),
        VfsError::WouldBlock => Errno::Again,
    }
}

fn unsupported(operation: &str, argument: impl core::fmt::Display) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
