use core::mem::{align_of, offset_of, size_of};

use roxy_fd::{Fd, FileError, FileMetadata};
use roxy_vfs::{Metadata as VfsMetadata, VfsError};

use super::{AtFlags, DirectoryFd, FileKind, unsupported};
use crate::{
    SyscallResult,
    args::{CString, Out, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::Stat, handle(target: StatTarget => Invalid, raw_fd: u64, path: u64, flags: AtFlags => Invalid, output: Out<StatAbi> => Fault));

const BLOCK_SIZE: u32 = 4096;

/// Fixed-layout stat payload copied across the userspace syscall ABI.
///
/// Layout per `sysdeps/roxy/sysdeps/filesystem.cpp`; offsets pinned by the assertions below.
#[repr(C)]
struct StatAbi {
    file_id: u64,
    size: u64,
    blocks: u64,
    hard_links: u64,
    /// The kind of file, one [`FileKind`] word.
    kind: u32,
    /// The permission bits the filesystem stores.
    ///
    /// These keep the POSIX `rwxrwxrwx`-plus-special-bit numbering: `chmod`, `mkdir`, and `open`
    /// pass them and the filesystem stores them as they arrive, so the kernel holds no second
    /// encoding to convert from.
    /// TODO(missing-capability: no owned permission model): give the record Roxy's own rights word,
    /// so a foreign mode word can be told apart from one of ours.
    permissions: u32,
    block_size: u32,
    /// Always zero. The record's eight-byte alignment leaves four bytes after `block_size`; naming
    /// them keeps its layout free of implicit padding.
    reserved: u32,
}

const _: () = assert!(size_of::<StatAbi>() == 48);
const _: () = assert!(align_of::<StatAbi>() == 8);
const _: () = assert!(offset_of!(StatAbi, file_id) == 0);
const _: () = assert!(offset_of!(StatAbi, size) == 8);
const _: () = assert!(offset_of!(StatAbi, blocks) == 16);
const _: () = assert!(offset_of!(StatAbi, hard_links) == 24);
const _: () = assert!(offset_of!(StatAbi, kind) == 32);
const _: () = assert!(offset_of!(StatAbi, permissions) == 36);
const _: () = assert!(offset_of!(StatAbi, block_size) == 40);
const _: () = assert!(offset_of!(StatAbi, reserved) == 44);

impl StatAbi {
    fn new(file_id: u64, size: u64, hard_links: u32, kind: FileKind, permissions: u32) -> Self {
        Self {
            file_id,
            size,
            blocks: size.div_ceil(512),
            hard_links: u64::from(hard_links),
            kind: kind.word(),
            permissions,
            block_size: BLOCK_SIZE,
            reserved: 0,
        }
    }
}

/// The `stat` target word, mirroring upstream mlibc's `fsfd_target` ordering.
///
/// mlibc passes its `fsfd_target` enumeration value through unchanged, so this word is a copy of
/// that enumeration rather than a numbering Roxy owns: `none` (0) and every value above `fd_path`
/// are requests no target names, which the parser reports through the error its registration
/// declares.
///
/// `path` and `fd_path` both name a file by path. `fd_path` additionally carries a directory
/// selector, which Roxy serves only for the working-directory selector `AT_FDCWD`; `path` ignores
/// its selector instead, because mlibc's `stat` and `lstat` pass -1 there with no meaning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatTarget {
    Path,
    Fd,
    FdPath,
}

impl StatTarget {
    /// The `AT_*` flags this target serves.
    ///
    /// A path target resolves the final component unless `AT_SYMLINK_NOFOLLOW` says otherwise. A
    /// descriptor target is told nothing about the path its descriptor was opened through, so it
    /// serves no flag of the word and rejects every one instead of dropping it.
    const fn accepted_flags(self) -> AtFlags {
        match self {
            Self::Path | Self::FdPath => AtFlags::SYMLINK_NOFOLLOW,
            Self::Fd => AtFlags::empty(),
        }
    }
}

impl SyscallArg for StatTarget {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        match raw {
            1 => Ok(Self::Path),
            2 => Ok(Self::Fd),
            3 => Ok(Self::FdPath),
            _ => Err(crate::unsupported::unsupported_argument(
                "stat.target",
                raw,
                error,
            )),
        }
    }
}

/// Dispatches one `stat` request.
///
/// `raw_fd` is the target's own word and stays raw until the target has selected its meaning: it is
/// a descriptor for the descriptor target, a directory selector for the directory-selector target,
/// and ignored for the path target.
fn handle(
    target: StatTarget,
    raw_fd: u64,
    path: u64,
    flags: AtFlags,
    output: Out<StatAbi>,
) -> SyscallResult {
    flags.require_only(target.accepted_flags(), "stat.flags")?;

    let result = match target {
        StatTarget::Path => path_metadata(path, !flags.contains(AtFlags::SYMLINK_NOFOLLOW))?,
        StatTarget::FdPath => {
            DirectoryFd::parse(raw_fd, Errno::Invalid)?
                .require_cwd("stat.dirfd", "stat.dirfd.foreign")?;

            path_metadata(path, !flags.contains(AtFlags::SYMLINK_NOFOLLOW))?
        }
        StatTarget::Fd => fd_metadata(raw_fd)?,
    };

    // SAFETY: StatAbi's checked repr(C) layout consists of initialized integer fields without
    // implicit padding.
    unsafe { output.write(&result) }?;

    Ok(0)
}

fn path_metadata(path: u64, follow_symlink: bool) -> Result<StatAbi, Errno> {
    let path = CString::parse(path, Errno::Fault)?;

    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    let metadata = if follow_symlink {
        roxy_vfs::metadata(path.into_inner())
    } else {
        roxy_vfs::symlink_metadata(path.into_inner())
    };

    metadata.map(StatAbi::from).map_err(map_vfs_error)
}

fn fd_metadata(raw: u64) -> Result<StatAbi, Errno> {
    let fd = Fd::parse(raw, Errno::BadFd)?;
    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;

    file.metadata()
        .map(StatAbi::from)
        .map_err(|error| match error {
            FileError::WouldBlock => Errno::Again,
            FileError::BadOperation => unsupported("stat.fd-object", raw),
            FileError::BrokenPipe => Errno::BrokenPipe,
            FileError::NotConnected => Errno::NotConnected,
            FileError::Io => Errno::Io,
            FileError::Interrupted => Errno::Interrupted,
        })
}

impl From<VfsMetadata> for StatAbi {
    fn from(metadata: VfsMetadata) -> Self {
        Self::new(
            metadata.file_id,
            metadata.size,
            metadata.hard_links,
            FileKind::from(metadata.file_type),
            u32::from(metadata.permissions.bits()),
        )
    }
}

impl From<FileMetadata> for StatAbi {
    fn from(metadata: FileMetadata) -> Self {
        Self::new(
            metadata.file_id,
            metadata.size,
            metadata.hard_links,
            FileKind::from(metadata.file_type),
            u32::from(metadata.permissions),
        )
    }
}

fn map_vfs_error(error: VfsError) -> Errno {
    match error {
        VfsError::NotFound => Errno::NotFound,
        VfsError::NotDirectory => Errno::NotDirectory,
        VfsError::PermissionDenied => Errno::Access,
        VfsError::InvalidPath | VfsError::InvalidInput => Errno::Invalid,
        VfsError::Unsupported => unsupported("stat.filesystem", 0),
        _ => Errno::Io,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::{FileMetadata, FileType};
    use roxy_test::kernel_test;

    use super::super::{AtFlags, FileKind};
    use super::{StatAbi, StatTarget};
    use crate::{args::SyscallArg, errno::Errno};

    kernel_test!("roxy-syscall::stat-encoding", stat_encoding, {
        let result = StatAbi::from(FileMetadata {
            file_id: 7,
            file_type: FileType::Regular,
            permissions: 0o640,
            size: 513,
            hard_links: 2,
        });

        assert_eq!(result.file_id, 7);
        assert_eq!(result.blocks, 2);
        // The kind and the permission bits are separate fields, so neither can be read as part of
        // the other; before they shared a word, a caller testing for a permission bit could match
        // a file kind's bits.
        assert_eq!(result.kind, FileKind::Regular.word());
        assert_eq!(result.permissions, 0o640);
        assert_eq!(result.reserved, 0);
        assert_eq!(result.hard_links, 2);
    });

    kernel_test!("roxy-syscall::stat-target", parses_the_named_targets, {
        // `fsfd_target`'s `path`, `fd`, and `fd_path`; `none` (0) and anything above `fd_path`
        // name no target and are reported through the error the registration declares.
        assert_eq!(StatTarget::parse(1, Errno::Invalid), Ok(StatTarget::Path));
        assert_eq!(StatTarget::parse(2, Errno::Invalid), Ok(StatTarget::Fd));
        assert_eq!(StatTarget::parse(3, Errno::Invalid), Ok(StatTarget::FdPath));
        assert_eq!(StatTarget::parse(0, Errno::Invalid), Err(Errno::Invalid));
        assert_eq!(StatTarget::parse(4, Errno::Invalid), Err(Errno::Invalid));
    });

    kernel_test!(
        "roxy-syscall::stat-flags",
        narrows_to_the_flags_each_target_serves,
        {
            // A path target resolves the final component unless the flag says otherwise.
            assert_eq!(StatTarget::Path.accepted_flags(), AtFlags::SYMLINK_NOFOLLOW);
            assert_eq!(
                StatTarget::FdPath.accepted_flags(),
                AtFlags::SYMLINK_NOFOLLOW
            );

            // `fstat` is told nothing about the path its descriptor was opened through.
            assert_eq!(StatTarget::Fd.accepted_flags(), AtFlags::empty());
        }
    );
}
