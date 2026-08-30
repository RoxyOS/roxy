mod accept;
mod bind;
mod connect;
mod create;
mod getsockopt;
mod listen;
mod msg;
mod peername;
mod recvmsg;
mod sendmsg;
mod shutdown;
mod sockname;

use alloc::vec::Vec;
use core::mem::{align_of, offset_of, size_of};

use roxy_fd::{FileError, SocketError};
use roxy_memory::UserAddress;
use roxy_vfs::{ResolvedPath, VfsError};

use crate::args::{Slice, user_memory};
use crate::errno::Errno;

pub(super) const SOCKET_SYSCALL: crate::Syscall = create::SYSCALL;
pub(super) const BIND_SYSCALL: crate::Syscall = bind::SYSCALL;
pub(super) const LISTEN_SYSCALL: crate::Syscall = listen::SYSCALL;
pub(super) const ACCEPT_SYSCALL: crate::Syscall = accept::SYSCALL;
pub(super) const CONNECT_SYSCALL: crate::Syscall = connect::SYSCALL;
pub(super) const SHUTDOWN_SYSCALL: crate::Syscall = shutdown::SYSCALL;
pub(super) const GETSOCKNAME_SYSCALL: crate::Syscall = sockname::SYSCALL;
pub(super) const GETPEERNAME_SYSCALL: crate::Syscall = peername::SYSCALL;
pub(super) const GETSOCKOPT_SYSCALL: crate::Syscall = getsockopt::SYSCALL;
pub(super) const RECVMSG_SYSCALL: crate::Syscall = recvmsg::SYSCALL;
pub(super) const SENDMSG_SYSCALL: crate::Syscall = sendmsg::SYSCALL;

/// Maps a `File` I/O error to its errno value for the `recvmsg`/`sendmsg` syscalls.
fn map_file_error(error: FileError) -> Errno {
    match error {
        FileError::WouldBlock => Errno::Again,
        FileError::BadOperation => Errno::BadFd,
        FileError::BrokenPipe => Errno::Pipe,
        FileError::NotConnected => Errno::NotConnected,
        FileError::Io => Errno::Io,
    }
}
const FAMILY_UNIX: u16 = 1;
const FAMILY_LENGTH: usize = size_of::<u16>();
const PATH_MAX: usize = 108;

/// The filesystem `sockaddr_un` record.
///
/// `bind` and `connect` receive a length-bounded prefix of this record: `sun_path` may end at any
/// offset within the caller-provided length, and decoding never reads bytes beyond that length
/// because callers are not required to map or initialize the record tail.
#[repr(C)]
struct SockaddrUn {
    sun_family: u16,
    sun_path: [u8; PATH_MAX],
}

const _: () = assert!(size_of::<SockaddrUn>() == FAMILY_LENGTH + PATH_MAX);
const _: () = assert!(align_of::<SockaddrUn>() == FAMILY_LENGTH);
const _: () = assert!(offset_of!(SockaddrUn, sun_path) == FAMILY_LENGTH);

/// Decodes a filesystem `sockaddr_un` record into its normalized absolute path.
///
/// The record is a `sun_family: u16` followed by an embedded `sun_path` byte string. The path
/// ends at the record length or the first embedded NUL, so callers may pass either a
/// length-bounded or a NUL-terminated address. Abstract-socket addresses (leading NUL) are not
/// supported. The returned path is normalized through the VFS path boundary, which resolves
/// relative addresses against the working directory.
///
/// # Errors
///
/// Returns `Invalid` for malformed records, reports unsupported address families through the
/// centralized diagnostic, and maps VFS normalization errors to their errno values.
fn decode_socket_path(address: UserAddress, length: u64) -> Result<Vec<u8>, Errno> {
    // Parse: the record is a family field followed by an embedded path byte string, bounded by
    // the caller-provided length and the maximum record size.
    let length = usize::try_from(length).map_err(|_| Errno::Invalid)?;

    if !(FAMILY_LENGTH + 1..=size_of::<SockaddrUn>()).contains(&length) {
        return Err(Errno::Invalid);
    }

    // SAFETY: u16 has a stable layout, no padding, and accepts every userspace-supplied bit
    // pattern. The family field lies within the validated record length.
    let mut family = 0u16;
    unsafe { user_memory::read(address, &mut family) }?;

    if family != FAMILY_UNIX {
        return Err(unsupported("socket.family", family));
    }

    // The path is an embedded byte string rather than a structured field, so it is copied as a
    // path-sized slice, exactly like path arguments elsewhere in this subsystem.
    let path_length = length - FAMILY_LENGTH;
    let path_address = address
        .checked_add(u64::try_from(FAMILY_LENGTH).map_err(|_| Errno::Fault)?)
        .ok_or(Errno::Fault)?;
    let path = Slice::<u8>::new(path_address, path_length);

    // SAFETY: u8 accepts every userspace-supplied byte pattern, and the slice lies within the
    // validated record length.
    let raw_path = unsafe { path.read() }?;

    // Check: truncate the path at an embedded NUL and reject empty addresses.
    let raw_path = match raw_path.iter().position(|byte| *byte == 0) {
        Some(terminator) => &raw_path[..terminator],
        None => raw_path.as_slice(),
    };

    if raw_path.is_empty() {
        return Err(Errno::Invalid);
    }

    // Implement: normalize through the VFS path boundary so relative addresses resolve against
    // the working directory.
    ResolvedPath::resolve(raw_path)
        .map(|resolved| resolved.as_bytes().to_vec())
        .map_err(map_vfs_error)
}

/// Encodes a normalized absolute path (or `None` for an unnamed socket) back into a `sockaddr_un`
/// record in userspace, writing the `sun_family` field followed by the `sun_path` byte string.
///
/// The caller provides the maximum writable record length; the actual length written is returned
/// so the syscall can report it through its `socklen_t` output. An unnamed socket writes only the
/// family field (Linux reports the family for anonymous `AF_UNIX` endpoints).
///
/// # Errors
///
/// Returns `TooBig` when `max_length` cannot hold the full record, and `Fault` when the record
/// cannot be written.
fn encode_socket_path(
    address: UserAddress,
    max_length: u64,
    path: Option<&[u8]>,
) -> Result<usize, Errno> {
    let max_length = usize::try_from(max_length).map_err(|_| Errno::Invalid)?;

    let path_length = path.map_or(0, <[u8]>::len);
    let total_length = FAMILY_LENGTH + path_length;

    if total_length > max_length {
        return Err(Errno::TooBig);
    }

    // SAFETY: u16 has a stable layout and every bit pattern is valid; the family field lies within
    // the validated writable range.
    let family = FAMILY_UNIX;
    unsafe { user_memory::write(address, &family) }?;

    if let Some(path) = path {
        let path_address = address
            .checked_add(u64::try_from(FAMILY_LENGTH).map_err(|_| Errno::Fault)?)
            .ok_or(Errno::Fault)?;

        // SAFETY: u8 accepts every byte pattern and the slice is bounded by `max_length`.
        unsafe { user_memory::write_slice(path_address, path) }?;
    }

    Ok(total_length)
}

fn map_socket_error(error: SocketError) -> Errno {
    match error {
        SocketError::AddressInUse => Errno::AddressInUse,
        SocketError::AlreadyConnected => Errno::AlreadyConnected,
        SocketError::ConnectionRefused => Errno::ConnectionRefused,
        SocketError::InvalidState => Errno::Invalid,
        SocketError::Io => Errno::Io,
    }
}

fn map_vfs_error(error: VfsError) -> Errno {
    match error {
        VfsError::NotInitialized | VfsError::Io | VfsError::Corrupt => Errno::Io,
        VfsError::InvalidPath | VfsError::InvalidInput => Errno::Invalid,
        _ => unsupported("socket.path", error),
    }
}

fn unsupported(operation: &str, argument: impl core::fmt::Display) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
