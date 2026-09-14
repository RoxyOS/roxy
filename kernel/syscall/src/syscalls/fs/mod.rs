mod dir;
mod link;
mod misc;
mod truncate;

use bitflags::bitflags;
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

/// Roxy's `dirfd` word: a descriptor plus one magic selector, not a numbered namespace.
///
/// A descriptor is never negative, so the working directory is spelled as a negative value no
/// descriptor can hold. Linux's own selector (-100) and every other negative is another
/// personality's numbering, which [`DirectoryFd::require_cwd`] reports as foreign.
const AT_FDCWD: i64 = -0x200;

/// Base of the Roxy `AT_*` flag word.
///
/// It sits above Linux's whole range for the word, whose top is `AT_RECURSIVE` at bit 15, so no
/// Linux flag can alias one of ours: a word below the base is another personality's numbering,
/// which [`AtFlags`]'s parser reports as foreign.
const AT_FLAGS_BASE: u64 = 1 << 16;

/// The base is above every Linux `AT_*` flag, so a word below it is never ours.
const _: () = assert!(AT_FLAGS_BASE > 0x8000);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DirectoryFd(i64);

impl DirectoryFd {
    /// Accepts only the working-directory selector, reporting every other value through the
    /// centralized diagnostic.
    ///
    /// `operation` names the argument for a value that could be a descriptor to resolve from, and
    /// `foreign` for a negative value, which is another personality's numbering: Linux spells the
    /// working directory as -100. An open descriptor and a value that is not a descriptor at all
    /// take the first path alike, because Roxy resolves no path relative to a descriptor and so
    /// never has to tell them apart.
    pub(super) fn require_cwd(self, operation: &str, foreign: &str) -> Result<(), Errno> {
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

bitflags! {
    /// The `AT_*` flag word the path syscalls share.
    ///
    /// Every flag the header defines is one bit from [`AT_FLAGS_BASE`], including the ones no
    /// carrier accepts yet. A carrier narrows the word to the flags it serves and reports the
    /// rest through the centralized diagnostic, so a flag of ours is never read as foreign and no
    /// request is dropped silently.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct AtFlags: u64 {
        const SYMLINK_NOFOLLOW = AT_FLAGS_BASE;
        const REMOVE_DIR = AT_FLAGS_BASE << 1;
        const SYMLINK_FOLLOW = AT_FLAGS_BASE << 2;
        const EACCESS = AT_FLAGS_BASE << 3;
    }
}

impl AtFlags {
    /// Rejects every set flag outside `accepted`, naming `operation` in the diagnostic.
    ///
    /// A carrier that serves no flag of the word passes `AtFlags::empty()`.
    pub(super) fn require_only(self, accepted: Self, operation: &str) -> Result<(), Errno> {
        let rejected = self.bits() & !accepted.bits();

        if rejected != 0 {
            return Err(unsupported_at_flags(operation, rejected));
        }

        Ok(())
    }
}

impl SyscallArg for AtFlags {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        // Anything below the base is another personality's numbering.
        let foreign = raw & (AT_FLAGS_BASE - 1);

        if foreign != 0 {
            return Err(unsupported_at_flags("at-flags.foreign", foreign));
        }

        let unknown = raw & !Self::all().bits();

        if unknown != 0 {
            return Err(unsupported_at_flags("at-flags.unknown", unknown));
        }

        Ok(Self::from_bits_retain(raw))
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

/// Reports a request Roxy cannot serve through the centralized diagnostic.
fn unsupported(operation: &str, argument: impl core::fmt::Display) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}

/// Reports an unusable `AT_*` flag word through the centralized diagnostic.
///
/// `EINVAL` rather than [`unsupported`]'s `ENOTSUP`: that is what `fstatat`, `unlinkat`, and
/// `linkat` answer an unusable flag word with on the personality Roxy mirrors, and `unlinkat`
/// already returned it. The word is shared, so its judgement has to give every carrier the same
/// answer.
fn unsupported_at_flags(operation: &str, argument: impl core::fmt::Display) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::Invalid)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{AT_FDCWD, AT_FLAGS_BASE, AtFlags, DirectoryFd};
    use crate::{args::SyscallArg, errno::Errno};

    kernel_test!(
        "roxy-syscall::at-fdcwd",
        only_the_magic_selector_names_the_cwd,
        {
            assert_eq!(
                DirectoryFd::parse(AT_FDCWD.cast_unsigned(), Errno::Invalid)
                    .unwrap()
                    .require_cwd("mkdirat.dirfd", "mkdirat.dirfd.foreign"),
                Ok(())
            );

            // A descriptor stays a descriptor whatever its number: 0x100 is what a base-based
            // numbering would have read as the working directory, and doing so would act on the
            // wrong directory while reporting success.
            assert_eq!(
                DirectoryFd::parse(0x100, Errno::Invalid)
                    .unwrap()
                    .require_cwd("mkdirat.dirfd", "mkdirat.dirfd.foreign"),
                Err(Errno::NotSupported)
            );

            // Linux spells the working directory as -100, which is another personality's
            // numbering.
            assert_eq!(
                DirectoryFd::parse((-100_i64).cast_unsigned(), Errno::Invalid)
                    .unwrap()
                    .require_cwd("mkdirat.dirfd", "mkdirat.dirfd.foreign"),
                Err(Errno::NotSupported)
            );
        }
    );

    kernel_test!("roxy-syscall::at-flags", parses_the_owned_flag_word, {
        assert_eq!(AtFlags::parse(0, Errno::Invalid), Ok(AtFlags::empty()));
        assert_eq!(
            AtFlags::parse(AT_FLAGS_BASE, Errno::Invalid),
            Ok(AtFlags::SYMLINK_NOFOLLOW)
        );
        assert_eq!(
            AtFlags::parse((AT_FLAGS_BASE << 1) | (AT_FLAGS_BASE << 2), Errno::Invalid),
            Ok(AtFlags::REMOVE_DIR | AtFlags::SYMLINK_FOLLOW)
        );

        // Linux defines `AT_SYMLINK_NOFOLLOW` as `0x100` and `AT_REMOVEDIR` as `0x200`; both are
        // below the base, so both are another personality's numbering rather than a flag of ours.
        assert_eq!(AtFlags::parse(0x100, Errno::Invalid), Err(Errno::Invalid));
        assert_eq!(AtFlags::parse(0x200, Errno::Invalid), Err(Errno::Invalid));

        // A bit of ours that no flag defines is a request of ours, reported as such.
        assert_eq!(
            AtFlags::parse(AT_FLAGS_BASE << 4, Errno::Invalid),
            Err(Errno::Invalid)
        );
    });

    kernel_test!(
        "roxy-syscall::at-flags",
        narrows_to_the_flags_a_carrier_serves,
        {
            // `stat` serves only `AT_SYMLINK_NOFOLLOW`.
            assert_eq!(
                AtFlags::SYMLINK_NOFOLLOW.require_only(AtFlags::SYMLINK_NOFOLLOW, "stat.flags"),
                Ok(())
            );
            assert_eq!(
                (AtFlags::SYMLINK_NOFOLLOW | AtFlags::REMOVE_DIR)
                    .require_only(AtFlags::SYMLINK_NOFOLLOW, "stat.flags"),
                Err(Errno::Invalid)
            );

            // A carrier that serves no flag of the word, such as `fstat`, rejects every one.
            assert_eq!(
                AtFlags::SYMLINK_NOFOLLOW.require_only(AtFlags::empty(), "stat.flags"),
                Err(Errno::Invalid)
            );
            assert_eq!(
                AtFlags::empty().require_only(AtFlags::empty(), "stat.flags"),
                Ok(())
            );
        }
    );
}
