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

/// Roxy's `dirfd` word is a descriptor plus one magic selector, not a numbered namespace: a
/// descriptor is never negative, so the working directory is spelled as a negative value that no
/// descriptor can hold. Linux's own selector (-100) and every other negative is another
/// personality's numbering, which `require_cwd` reports as foreign.
const AT_FDCWD: i64 = -0x200;

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

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{AT_FDCWD, DirectoryFd};
    use crate::errno::Errno;

    kernel_test!(
        "roxy-syscall::at-fdcwd",
        only_the_magic_selector_names_the_cwd,
        {
            assert_eq!(
                DirectoryFd(AT_FDCWD).require_cwd("mkdirat.dirfd", "mkdirat.dirfd.foreign"),
                Ok(())
            );

            // A descriptor stays a descriptor whatever its number: 0x100 is what a base-based
            // numbering would have read as the working directory, and doing so would act on the wrong
            // directory while reporting success.
            assert_eq!(
                DirectoryFd(0x100).require_cwd("mkdirat.dirfd", "mkdirat.dirfd.foreign"),
                Err(Errno::NotSupported)
            );

            // Linux spells the working directory as -100, which is another personality's numbering.
            assert_eq!(
                DirectoryFd(-100).require_cwd("mkdirat.dirfd", "mkdirat.dirfd.foreign"),
                Err(Errno::NotSupported)
            );
        }
    );
}
