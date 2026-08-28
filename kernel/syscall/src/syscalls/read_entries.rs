use alloc::vec::Vec;
use core::mem;

use roxy_fd::{DirectoryEntry, Fd, FileError, FileType};
use roxy_memory::UserAddress;

use crate::{SyscallResult, args::Slice, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::ReadEntries, handle(fd: Fd => BadFd, address: UserAddress => Fault, max_size: usize => Fault));

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

fn handle(fd: Fd, address: UserAddress, max_size: usize) -> SyscallResult {
    if max_size < DIRENT_SIZE {
        return Err(Errno::Invalid);
    }

    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;
    Slice::<u8>::new(address, max_size).validate_writable()?;

    let output = Slice::<DirentAbi>::new(address, max_size / DIRENT_SIZE);
    let entries = file
        .read_directory_entries(max_size / DIRENT_SIZE)
        .map_err(map_file_error)?;
    let encoded = encode_entries(&entries)?;

    // SAFETY: DirentAbi's repr(C) layout explicitly represents trailing padding, and encode_entry
    // initializes every integer, byte-array, and padding field.
    unsafe { output.write(&encoded) }?;

    Ok(u64::try_from(mem::size_of_val(encoded.as_slice())).unwrap())
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
        FileError::BrokenPipe => Errno::Pipe,
        FileError::NotConnected => Errno::NotConnected,
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
