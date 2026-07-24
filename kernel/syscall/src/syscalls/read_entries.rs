use alloc::vec::Vec;
use core::{mem, slice};

use roxy_fd::{DirectoryEntry, Fd, FileError, FileType};
use roxy_memory::UserAddress;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::ReadEntries, handle);

const DIRENT_RECORD_SIZE: u16 = 280;
const NAME_SIZE: usize = 256;

#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct DirentAbi {
    inode: u64,
    offset: i64,
    record_size: u16,
    file_type: u8,
    name: [u8; NAME_SIZE],
    padding: [u8; 5],
}

const DIRENT_SIZE: usize = mem::size_of::<DirentAbi>();
const _: () = assert!(DIRENT_SIZE == 280);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let fd = u32::try_from(arguments[0])
        .map(Fd::new)
        .map_err(|_| Errno::BadFd)?;
    let address = UserAddress::new(arguments[1]).ok_or(Errno::Fault)?;
    let max_size = usize::try_from(arguments[2]).map_err(|_| Errno::Fault)?;

    if max_size < DIRENT_SIZE {
        return Err(Errno::Invalid);
    }

    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;
    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;

    addrspace
        .validate_writable(address, max_size)
        .map_err(|_| Errno::Fault)?;

    let entries = file
        .read_directory_entries(max_size / DIRENT_SIZE)
        .map_err(map_file_error)?;
    let output = encode_entries(&entries)?;

    write_entries(&addrspace, address, &output)?;

    Ok(u64::try_from(mem::size_of_val(output.as_slice())).unwrap())
}

fn encode_entries(entries: &[DirectoryEntry]) -> Result<Vec<DirentAbi>, Errno> {
    entries.iter().map(encode_entry).collect()
}

fn encode_entry(entry: &DirectoryEntry) -> Result<DirentAbi, Errno> {
    if entry.name.len() >= NAME_SIZE {
        return Err(Errno::Io);
    }

    let mut name = [0; NAME_SIZE];
    name[..entry.name.len()].copy_from_slice(&entry.name);

    Ok(DirentAbi {
        inode: entry.file_id,
        offset: i64::try_from(entry.offset).map_err(|_| Errno::Overflow)?,
        record_size: DIRENT_RECORD_SIZE,
        file_type: encode_file_type(entry.file_type),
        name,
        padding: [0; 5],
    })
}

fn write_entries(
    addrspace: &roxy_vm::AddrSpaceHandle,
    address: UserAddress,
    entries: &[DirentAbi],
) -> Result<(), Errno> {
    // SAFETY: DirentAbi is repr(C), explicitly includes and initializes all ABI padding, and the
    // byte slice does not outlive the borrowed entries.
    let bytes =
        unsafe { slice::from_raw_parts(entries.as_ptr().cast::<u8>(), mem::size_of_val(entries)) };

    addrspace
        .write_bytes(address, bytes)
        .map_err(|_| Errno::Fault)
}

const fn encode_file_type(file_type: FileType) -> u8 {
    match file_type {
        FileType::Fifo => 1,
        FileType::CharacterDevice => 2,
        FileType::Directory => 4,
        FileType::BlockDevice => 6,
        FileType::Regular => 8,
        FileType::Symlink => 10,
        FileType::Socket => 12,
        FileType::Unknown => 0,
    }
}

fn map_file_error(error: FileError) -> Errno {
    match error {
        FileError::BadOperation => Errno::NotDirectory,
        FileError::Io => Errno::Io,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;
    use roxy_fd::{DirectoryEntry, FileType};
    use roxy_test::kernel_test;

    use super::{DIRENT_SIZE, encode_entries};

    kernel_test!("roxy-syscall::directory-entry-encoding", encodes_entry, {
        let encoded = encode_entries(&[DirectoryEntry {
            file_id: 42,
            offset: 7,
            file_type: FileType::Directory,
            name: vec![b'a'],
        }])
        .unwrap();

        assert_eq!(DIRENT_SIZE, 280);
        assert_eq!(encoded.len(), 1);
        assert_eq!(encoded[0].inode, 42);
        assert_eq!(encoded[0].offset, 7);
        assert_eq!(encoded[0].record_size, 280);
        assert_eq!(encoded[0].file_type, 4);
        assert_eq!(&encoded[0].name[..2], b"a\0");
        assert_eq!(encoded[0].padding, [0; 5]);
    });
}
