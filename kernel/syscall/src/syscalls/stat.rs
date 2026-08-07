use core::mem::{align_of, offset_of, size_of};

use bitflags::bitflags;
use roxy_fd::{Fd, FileError, FileMetadata, FileType as FdFileType};
use roxy_vfs::{FileType as VfsFileType, Metadata as VfsMetadata, VfsError};

use crate::{
    SyscallResult,
    args::{CString, Out, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::Stat, handle(target: StatTarget => Invalid, fd: u64, path: u64, flags: StatFlags => Invalid, output: Out<StatAbi> => Fault));

const BLOCK_SIZE: u32 = 4096;
const MODE_REGULAR: u32 = 0x8000;
const MODE_DIRECTORY: u32 = 0x4000;
const MODE_SYMLINK: u32 = 0xa000;
const MODE_BLOCK: u32 = 0x6000;
const MODE_CHARACTER: u32 = 0x2000;
const MODE_FIFO: u32 = 0x1000;
const MODE_SOCKET: u32 = 0xc000;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct StatFlags: u64 {
        const SYMLINK_NOFOLLOW = 0x100;
    }
}

impl SyscallArg for StatFlags {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        let unknown = raw & !Self::all().bits();

        if unknown != 0 {
            return Err(unsupported("stat.flags", unknown));
        }

        Ok(Self::from_bits_retain(raw))
    }
}

/// Fixed-layout stat payload copied across the userspace syscall ABI.
#[repr(C)]
struct StatAbi {
    file_id: u64,
    size: u64,
    blocks: u64,
    hard_links: u64,
    mode: u32,
    block_size: u32,
}

const _: () = assert!(size_of::<StatAbi>() == 40);
const _: () = assert!(align_of::<StatAbi>() == 8);
const _: () = assert!(offset_of!(StatAbi, file_id) == 0);
const _: () = assert!(offset_of!(StatAbi, size) == 8);
const _: () = assert!(offset_of!(StatAbi, blocks) == 16);
const _: () = assert!(offset_of!(StatAbi, hard_links) == 24);
const _: () = assert!(offset_of!(StatAbi, mode) == 32);
const _: () = assert!(offset_of!(StatAbi, block_size) == 36);

impl StatAbi {
    fn new(file_id: u64, size: u64, hard_links: u32, mode: u32) -> Self {
        Self {
            file_id,
            size,
            blocks: size.div_ceil(512),
            hard_links: u64::from(hard_links),
            mode,
            block_size: BLOCK_SIZE,
        }
    }
}

#[derive(Clone, Copy)]
enum StatTarget {
    Path,
    Fd,
}

impl SyscallArg for StatTarget {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            1 => Ok(Self::Path),
            2 => Ok(Self::Fd),
            _ => Err(unsupported("stat.target", raw)),
        }
    }
}

fn handle(
    target: StatTarget,
    fd: u64,
    path: u64,
    flags: StatFlags,
    output: Out<StatAbi>,
) -> SyscallResult {
    let result = match target {
        StatTarget::Path => path_metadata(path, flags)?,
        StatTarget::Fd => fd_metadata(fd)?,
    };

    // SAFETY: StatAbi's checked repr(C) layout consists of initialized integer fields without
    // implicit padding.
    unsafe { output.write(&result) }?;

    Ok(0)
}

fn path_metadata(path: u64, flags: StatFlags) -> Result<StatAbi, Errno> {
    let path = CString::parse(path, Errno::Fault)?;

    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    let metadata = if flags.contains(StatFlags::SYMLINK_NOFOLLOW) {
        roxy_vfs::symlink_metadata(path.into_inner())
    } else {
        roxy_vfs::metadata(path.into_inner())
    };

    metadata.map(StatAbi::from).map_err(map_vfs_error)
}

fn fd_metadata(raw: u64) -> Result<StatAbi, Errno> {
    let fd = Fd::parse(raw, Errno::BadFd)?;
    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;

    file.metadata()
        .map(StatAbi::from)
        .map_err(|error| match error {
            FileError::BadOperation => unsupported("stat.fd-object", raw),
            FileError::BrokenPipe => Errno::Pipe,
            FileError::Io => Errno::Io,
        })
}

impl From<VfsMetadata> for StatAbi {
    fn from(metadata: VfsMetadata) -> Self {
        Self::new(
            metadata.file_id,
            metadata.size,
            metadata.hard_links,
            vfs_file_type(metadata.file_type) | u32::from(metadata.permissions.bits()),
        )
    }
}

impl From<FileMetadata> for StatAbi {
    fn from(metadata: FileMetadata) -> Self {
        Self::new(
            metadata.file_id,
            metadata.size,
            metadata.hard_links,
            fd_file_type(metadata.file_type) | u32::from(metadata.permissions),
        )
    }
}

fn vfs_file_type(file_type: VfsFileType) -> u32 {
    match file_type {
        VfsFileType::Regular => MODE_REGULAR,
        VfsFileType::Directory => MODE_DIRECTORY,
        VfsFileType::Symlink => MODE_SYMLINK,
        VfsFileType::BlockDevice => MODE_BLOCK,
        VfsFileType::CharacterDevice => MODE_CHARACTER,
        VfsFileType::Fifo => MODE_FIFO,
        VfsFileType::Socket => MODE_SOCKET,
        VfsFileType::Unknown => 0,
    }
}

fn fd_file_type(file_type: FdFileType) -> u32 {
    match file_type {
        FdFileType::Regular => MODE_REGULAR,
        FdFileType::Directory => MODE_DIRECTORY,
        FdFileType::Symlink => MODE_SYMLINK,
        FdFileType::BlockDevice => MODE_BLOCK,
        FdFileType::CharacterDevice => MODE_CHARACTER,
        FdFileType::Fifo => MODE_FIFO,
        FdFileType::Socket => MODE_SOCKET,
        FdFileType::Unknown => 0,
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

fn unsupported(operation: &str, argument: u64) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::{FileMetadata, FileType};
    use roxy_test::kernel_test;

    use super::{MODE_REGULAR, StatAbi, StatFlags};
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
        assert_eq!(result.mode, MODE_REGULAR | 0o640);
        assert_eq!(result.hard_links, 2);
    });

    kernel_test!("roxy-syscall::stat-flags", stat_flags, {
        assert_eq!(StatFlags::parse(0, Errno::Invalid), Ok(StatFlags::empty()));
        assert_eq!(
            StatFlags::parse(0x100, Errno::Invalid),
            Ok(StatFlags::SYMLINK_NOFOLLOW)
        );
    });
}
