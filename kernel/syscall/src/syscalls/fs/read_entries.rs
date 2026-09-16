use alloc::vec::Vec;
use core::mem;

use roxy_fd::{DirectoryEntry, Fd, FileError};
use roxy_memory::UserAddress;

use super::FileKind;
use crate::{SyscallResult, args::Slice, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::ReadEntries, handle(fd: Fd => BadFd, address: UserAddress => Fault, max_size: usize => Fault));

/// The bytes one entry name occupies in a record.
///
/// A record reports the name's length in one byte, so the longest name it carries is [`u8::MAX`] and
/// the array holds one byte more than that.
const NAME_SIZE: usize = 256;

const _: () = assert!(NAME_SIZE == u8::MAX as usize + 1);

/// One directory entry, copied across the userspace syscall ABI.
///
/// The record carries what the kernel knows about an entry — the file it names, the position the
/// directory resumes from, the length of its name, and the kind of file it is — and nothing about
/// how a caller walks or renders it. It has no record size and does not terminate its name, because
/// those are conventions of the reader rather than properties of the entry; the libc renders the
/// POSIX `struct dirent` its consumers read from a record at this boundary, the division of labour
/// the `stat` result and the `wait` status already follow. The layout is mirrored by
/// `sysdeps/roxy/sysdeps/filesystem.cpp`; the assertions below pin this side and that file's own
/// `offsetof` assertions pin the other.
///
/// `reserved` is always zero and makes the header's padding explicit: the struct then has no
/// implicit padding, so an array of records is its own wire format.
#[repr(C)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct DirentAbi {
    file_id: u64,
    /// The position the directory resumes from, one past this entry. It is an entry index rather
    /// than a byte offset, and it is what the libc reports as the POSIX `d_off` that `telldir`
    /// returns and `seekdir` seeks back to.
    offset: u64,
    /// The bytes of `name` that are the entry's name, which the record states instead of
    /// terminating the name.
    name_len: u8,
    /// The kind of file the entry describes, one [`FileKind`] byte.
    kind: u8,
    /// Always zero.
    reserved: [u8; 6],
    name: [u8; NAME_SIZE],
}

const DIRENT_SIZE: usize = mem::size_of::<DirentAbi>();
const _: () = assert!(DIRENT_SIZE == 280);
const _: () = assert!(mem::offset_of!(DirentAbi, file_id) == 0);
const _: () = assert!(mem::offset_of!(DirentAbi, offset) == 8);
const _: () = assert!(mem::offset_of!(DirentAbi, name_len) == 16);
const _: () = assert!(mem::offset_of!(DirentAbi, kind) == 17);
const _: () = assert!(mem::offset_of!(DirentAbi, reserved) == 18);
const _: () = assert!(mem::offset_of!(DirentAbi, name) == 24);
const _: () = assert!(mem::offset_of!(DirentAbi, name) + NAME_SIZE == DIRENT_SIZE);

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

    // SAFETY: DirentAbi's repr(C) layout explicitly represents its padding, and encode_entry
    // initializes every integer, byte-array, and reserved field.
    unsafe { output.write(&encoded) }?;

    Ok(u64::try_from(mem::size_of_val(encoded.as_slice())).unwrap())
}

fn encode_entries(entries: &[DirectoryEntry]) -> Result<Vec<DirentAbi>, Errno> {
    entries.iter().map(encode_entry).collect()
}

fn encode_entry(entry: &DirectoryEntry) -> Result<DirentAbi, Errno> {
    // A name too long for the record's one-byte length is rejected rather than truncated. The
    // conversion is what reports it, so the copy below only ever runs for a length that fits.
    let name_len = u8::try_from(entry.name.len()).map_err(|_| Errno::Io)?;

    let mut name = [0; NAME_SIZE];
    name[..entry.name.len()].copy_from_slice(&entry.name);

    Ok(DirentAbi {
        file_id: entry.file_id,
        offset: entry.offset,
        name_len,
        kind: FileKind::from(entry.file_type).byte(),
        reserved: [0; 6],
        name,
    })
}

fn map_file_error(error: FileError) -> Errno {
    match error {
        FileError::WouldBlock => Errno::Again,
        FileError::BadOperation => Errno::NotDirectory,
        FileError::BrokenPipe => Errno::BrokenPipe,
        FileError::NotConnected => Errno::NotConnected,
        FileError::Io => Errno::Io,
        FileError::Interrupted => Errno::Interrupted,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;
    use roxy_fd::{DirectoryEntry, FileType};
    use roxy_test::kernel_test;

    use super::super::FileKind;
    use super::{DIRENT_SIZE, NAME_SIZE, encode_entries};

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
        assert_eq!(encoded[0].file_id, 42);
        assert_eq!(encoded[0].offset, 7);
        assert_eq!(encoded[0].name_len, 1);
        // The kind is Roxy's own word, not POSIX's `d_type` value for a directory; the libc renders
        // the `d_type` byte userspace compares against.
        assert_eq!(encoded[0].kind, FileKind::Directory.byte());
        assert_ne!(encoded[0].kind, 4);
        assert_eq!(&encoded[0].name[..2], b"a\0");
        assert_eq!(encoded[0].reserved, [0; 6]);

        // A name the record's length cannot count is rejected rather than truncated, while one of
        // exactly that length is carried whole.
        let longest = vec![b'a'; NAME_SIZE - 1];
        let encoded = encode_entries(&[DirectoryEntry {
            file_id: 1,
            offset: 1,
            file_type: FileType::Regular,
            name: longest,
        }])
        .unwrap();
        assert_eq!(encoded[0].name_len, u8::MAX);

        let overlong = vec![b'a'; NAME_SIZE];
        assert!(
            encode_entries(&[DirectoryEntry {
                file_id: 1,
                offset: 1,
                file_type: FileType::Regular,
                name: overlong,
            }])
            .is_err()
        );
    });
}
