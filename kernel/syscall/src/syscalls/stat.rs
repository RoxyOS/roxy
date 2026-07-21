use core::{
    mem::{align_of, offset_of, size_of},
    slice,
};

use roxy_fd::{Fd, FileError, FileMetadata, FileType as FdFileType};
use roxy_memory::UserAddress;
use roxy_vfs::{FileType as VfsFileType, Metadata as VfsMetadata, ResolvedPath, VfsError};

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Stat, handle);

const BLOCK_SIZE: u32 = 4096;
const MODE_REGULAR: u32 = 0x8000;
const MODE_DIRECTORY: u32 = 0x4000;
const MODE_SYMLINK: u32 = 0xa000;
const MODE_BLOCK: u32 = 0x6000;
const MODE_CHARACTER: u32 = 0x2000;
const MODE_FIFO: u32 = 0x1000;
const MODE_SOCKET: u32 = 0xc000;

/// Fixed-layout stat payload copied across the userspace syscall ABI.
/// Not to be confused with `StatData`
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

#[derive(Clone, Copy)]
enum StatTarget {
    Path,
    Fd,
}

impl StatTarget {
    fn parse(value: u64) -> Result<Self, Errno> {
        match value {
            1 => Ok(Self::Path),
            2 => Ok(Self::Fd),
            _ => Err(unsupported("stat.target", value)),
        }
    }
}

/// Normalized kernel metadata shared by path-based and descriptor-based stat requests. Not to be
/// confused with `StatAbi`
#[derive(Clone, Copy)]
struct StatData {
    file_id: u64,
    file_type: u32,
    permissions: u16,
    size: u64,
    hard_links: u32,
}

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let target = StatTarget::parse(arguments[0])?;
    let output = UserAddress::new(arguments[4]).ok_or(Errno::Fault)?;

    if arguments[3] != 0 {
        return Err(unsupported("stat.flags", arguments[3]));
    }

    let metadata = match target {
        StatTarget::Path => path_metadata(arguments[2])?,
        StatTarget::Fd => fd_metadata(arguments[1])?,
    };
    let result = encode(metadata);

    write_result(output, &result)?;

    Ok(0)
}

fn path_metadata(path: u64) -> Result<StatData, Errno> {
    let address = UserAddress::new(path).ok_or(Errno::Fault)?;
    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;
    let path = crate::user::read_c_string(&addrspace, address, ResolvedPath::MAX_LEN)?;

    if path.is_empty() {
        return Err(Errno::NotFound);
    }

    roxy_vfs::metadata(path)
        .map(StatData::from)
        .map_err(map_vfs_error)
}

fn fd_metadata(raw: u64) -> Result<StatData, Errno> {
    let fd = u32::try_from(raw).map(Fd::new).map_err(|_| Errno::BadFd)?;
    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;

    file.metadata()
        .map(StatData::from)
        .map_err(|error| match error {
            FileError::BadOperation => unsupported("stat.fd-object", raw),
            FileError::Io => Errno::Io,
        })
}

fn encode(metadata: StatData) -> StatAbi {
    StatAbi {
        file_id: metadata.file_id,
        size: metadata.size,
        blocks: metadata.size.div_ceil(512),
        hard_links: u64::from(metadata.hard_links),
        mode: metadata.file_type | u32::from(metadata.permissions),
        block_size: BLOCK_SIZE,
    }
}

fn write_result(output: UserAddress, result: &StatAbi) -> Result<(), Errno> {
    // SAFETY: StatAbi is repr(C), contains only initialized integer fields, and the slice does
    // not outlive the borrowed value.
    let bytes = unsafe {
        slice::from_raw_parts(
            (core::ptr::from_ref(result)).cast::<u8>(),
            size_of::<StatAbi>(),
        )
    };
    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;

    addrspace
        .write_bytes(output, bytes)
        .map_err(|_| Errno::Fault)
}

impl From<VfsMetadata> for StatData {
    fn from(metadata: VfsMetadata) -> Self {
        Self {
            file_id: metadata.file_id,
            file_type: vfs_file_type(metadata.file_type),
            permissions: metadata.permissions.bits(),
            size: metadata.size,
            hard_links: metadata.hard_links,
        }
    }
}

impl From<FileMetadata> for StatData {
    fn from(metadata: FileMetadata) -> Self {
        Self {
            file_id: metadata.file_id,
            file_type: fd_file_type(metadata.file_type),
            permissions: metadata.permissions,
            size: metadata.size,
            hard_links: metadata.hard_links,
        }
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
    use roxy_test::kernel_test;

    use super::{MODE_REGULAR, StatData, encode};

    kernel_test!("roxy-syscall::stat-encoding", stat_encoding, {
        let result = encode(StatData {
            file_id: 7,
            file_type: MODE_REGULAR,
            permissions: 0o640,
            size: 513,
            hard_links: 2,
        });

        assert_eq!(result.file_id, 7);
        assert_eq!(result.blocks, 2);
        assert_eq!(result.mode, MODE_REGULAR | 0o640);
        assert_eq!(result.hard_links, 2);
    });
}
