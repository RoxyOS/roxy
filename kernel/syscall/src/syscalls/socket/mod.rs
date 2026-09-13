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

use roxy_fd::SocketError;
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

pub(super) use super::iovec::map_file_error;

/// The Roxy socket argument words. `socket` and `socketpair` take the same domain, type, and
/// protocol, so the words are judged here, once, rather than in a copy in each handler: a copy goes
/// stale exactly when the numbering changes, which is how `socketpair` came to refuse every caller
/// after `AF_UNIX` and `SOCK_STREAM` moved.
mod words {
    use super::{FamilyVerdict, classify_family};
    use crate::{args::SyscallArg, errno::Errno};

    /// The Roxy socket-type word follows `abi-bits/socket.h`: `SOCK_STREAM` is the first value of a
    /// three-bit type field at the base, the two supported flags sit above the field, and a value
    /// below the base is another personality's numbering. `SOCK_DGRAM` keeps a value of its own
    /// because upstream mlibc's `switch` names it as a case, and is reported as unsupported all the
    /// same.
    const SOCK_BASE: u64 = 1 << 20;
    const SOCK_TYPE_MASK: u64 = (SOCK_BASE << 3) - SOCK_BASE;
    const SOCK_UNSUPPORTED: u64 = 1 << 12;
    const SOCK_DGRAM: u64 = SOCK_BASE + 1;
    const SOCK_CLOEXEC: u64 = SOCK_BASE << 4;
    const SOCK_NONBLOCK: u64 = SOCK_BASE << 5;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(in crate::syscalls) enum Domain {
        Unix,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(in crate::syscalls) enum SocketType {
        Stream,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(in crate::syscalls) enum Protocol {
        Default,
    }

    /// Reports an unsupported argument word. `EINVAL` rather than `ENOTSUP`: `socket`'s contract
    /// answers a bad domain, type, or protocol that way, and callers such as libxcb retry without
    /// their descriptor flags exactly when they see `EINVAL`.
    fn unsupported_word(operation: &str, argument: impl core::fmt::Display) -> Errno {
        crate::unsupported::unsupported_argument(operation, argument, Errno::Invalid)
    }

    impl SyscallArg for Domain {
        fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
            match classify_family(raw) {
                FamilyVerdict::Served => Ok(Self::Unix),
                FamilyVerdict::Unsupported => {
                    Err(unsupported_word("socket.domain.unsupported", raw))
                }
                FamilyVerdict::Foreign => Err(unsupported_word("socket.domain.foreign", raw)),
                FamilyVerdict::Undefined => Err(unsupported_word("socket.domain", raw)),
            }
        }
    }

    impl SyscallArg for SocketType {
        fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
            // The marker is a bit, so a caller that ORs it into a supported value is still
            // recognised as asking for something Roxy cannot serve rather than as passing a foreign
            // number.
            if raw & SOCK_UNSUPPORTED != 0 || raw == SOCK_DGRAM {
                return Err(unsupported_word("socket.type.unsupported", raw));
            }

            if raw & (SOCK_CLOEXEC | SOCK_NONBLOCK) != 0 {
                return Err(unsupported_word("socket.descriptor-flags", raw));
            }

            // Anything below the base is another personality's numbering — Linux puts its type in
            // bits 0-3 and its descriptor flags in bits 11 and 19 — and anything this word does not
            // define, above the type field or beside it, is a request of ours that no flag names.
            // Both are reported; neither may pass through unread.
            if raw < SOCK_BASE {
                return Err(unsupported_word("socket.type.foreign", raw));
            }

            let unknown = raw & !(SOCK_TYPE_MASK | SOCK_CLOEXEC | SOCK_NONBLOCK);

            if unknown != 0 {
                return Err(unsupported_word("socket.type.unknown", unknown));
            }

            match raw & SOCK_TYPE_MASK {
                SOCK_BASE => Ok(Self::Stream),
                _ => Err(unsupported_word("socket.type", raw)),
            }
        }
    }

    impl SyscallArg for Protocol {
        fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
            match raw {
                0 => Ok(Self::Default),
                _ => Err(unsupported_word("socket.protocol", raw)),
            }
        }
    }

    #[cfg(feature = "kernel-test")]
    mod tests {
        use roxy_test::kernel_test;

        use super::super::{AF_INET, AF_UNIX, AF_UNSUPPORTED, FamilyVerdict, classify_family};
        use super::{
            Domain, Protocol, SOCK_BASE, SOCK_CLOEXEC, SOCK_DGRAM, SOCK_NONBLOCK, SOCK_UNSUPPORTED,
            SocketType,
        };
        use crate::args::SyscallArg;
        use crate::errno::Errno;

        kernel_test!(
            "roxy-syscall::socket-arguments",
            parses_supported_arguments,
            {
                assert_eq!(
                    Domain::parse(u64::from(AF_UNIX), Errno::Invalid),
                    Ok(Domain::Unix)
                );
                assert_eq!(
                    SocketType::parse(SOCK_BASE, Errno::Invalid),
                    Ok(SocketType::Stream)
                );
                assert_eq!(Protocol::parse(0, Errno::Invalid), Ok(Protocol::Default));
            }
        );

        kernel_test!(
            "roxy-syscall::socket-arguments",
            classifies_the_family_word,
            {
                assert_eq!(classify_family(u64::from(AF_UNIX)), FamilyVerdict::Served);
                assert_eq!(
                    classify_family(u64::from(AF_UNSUPPORTED)),
                    FamilyVerdict::Unsupported
                );
                assert_eq!(
                    classify_family(u64::from(AF_INET)),
                    FamilyVerdict::Unsupported
                );
                assert_eq!(classify_family(1), FamilyVerdict::Foreign);
                assert_eq!(
                    classify_family(u64::from(AF_UNIX) + 4),
                    FamilyVerdict::Undefined
                );
            }
        );

        kernel_test!(
            "roxy-syscall::socket-arguments",
            rejects_unsupported_arguments,
            {
                // Anything below the bases is another personality's numbering: Linux spells the
                // Unix family 1 and the stream type 1.
                assert_eq!(Domain::parse(1, Errno::Invalid), Err(Errno::Invalid));
                assert_eq!(SocketType::parse(1, Errno::Invalid), Err(Errno::Invalid));
                assert_eq!(Protocol::parse(6, Errno::Invalid), Err(Errno::Invalid));

                // The marker, and the three names that keep values of their own, are what the
                // header offers a caller who wants something Roxy does not serve; each is refused
                // whether it arrives alone or ORed into a supported value.
                assert_eq!(
                    Domain::parse(u64::from(AF_UNSUPPORTED), Errno::Invalid),
                    Err(Errno::Invalid)
                );
                assert_eq!(
                    Domain::parse(u64::from(AF_INET), Errno::Invalid),
                    Err(Errno::Invalid)
                );
                assert_eq!(
                    Domain::parse(
                        u64::from(AF_UNSUPPORTED) | u64::from(AF_UNIX),
                        Errno::Invalid
                    ),
                    Err(Errno::Invalid)
                );
                assert_eq!(
                    SocketType::parse(SOCK_UNSUPPORTED | SOCK_BASE, Errno::Invalid),
                    Err(Errno::Invalid)
                );
                assert_eq!(
                    SocketType::parse(SOCK_DGRAM, Errno::Invalid),
                    Err(Errno::Invalid)
                );
            }
        );

        kernel_test!(
            "roxy-syscall::socket-arguments",
            rejects_descriptor_flags,
            {
                let cloexec = SOCK_BASE | SOCK_CLOEXEC;
                let nonblocking = SOCK_BASE | SOCK_NONBLOCK;
                let unknown_flag = SOCK_BASE | (1 << 30);

                assert_eq!(
                    SocketType::parse(cloexec, Errno::Invalid),
                    Err(Errno::Invalid)
                );
                assert_eq!(
                    SocketType::parse(nonblocking, Errno::Invalid),
                    Err(Errno::Invalid)
                );
                assert_eq!(
                    SocketType::parse(unknown_flag, Errno::Invalid),
                    Err(Errno::Invalid)
                );
            }
        );
    }
}

pub(in crate::syscalls) use words::{Domain, Protocol, SocketType};

/// The Roxy `AF_*` values this subsystem judges, in one place and in the family field's own width,
/// because two arguments carry the family: the `socket(2)` domain and the `sockaddr_un.sun_family`
/// field. Numbered from a base above Linux's family range (`PF_MAX` 46), with the header's marker
/// for a family Roxy defines but cannot serve. See `abi-bits/socket.h`.
const AF_UNIX: u16 = 0x100;
const AF_UNSUPPORTED: u16 = 0x80;
const AF_INET: u16 = AF_UNIX + 1;
const AF_INET6: u16 = AF_UNIX + 2;
const FAMILY_LENGTH: usize = size_of::<u16>();

/// How a family word relates to what this subsystem serves.
///
/// The four cases are what the diagnostic has to tell apart: a family this kernel serves, one the
/// header defines but this kernel cannot serve, another personality's numbering, and a value of
/// our own numbering that no family defines. The errno is the same for the last three, so the
/// classification is also the unit a test can observe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FamilyVerdict {
    Served,
    Unsupported,
    Foreign,
    Undefined,
}

fn classify_family(family: u64) -> FamilyVerdict {
    if family == u64::from(AF_UNIX) {
        return FamilyVerdict::Served;
    }

    // `AF_INET` and `AF_INET6` keep values of their own instead of the marker, because upstream
    // mlibc switches on both and duplicate case labels would not compile; they are served no more
    // than the marker is. The marker is a bit, so a caller that ORs it into a supported value is
    // still read as asking for something Roxy cannot serve.
    if family & u64::from(AF_UNSUPPORTED) != 0
        || family == u64::from(AF_INET)
        || family == u64::from(AF_INET6)
    {
        return FamilyVerdict::Unsupported;
    }

    // The word is an enumeration numbered from the base, so its own values sit at or above it and
    // a range test is what separates another personality's numbering from ours: Linux's families
    // are below the base, while a value above it that no family defines stays ours to report.
    if family < u64::from(AF_UNIX) {
        return FamilyVerdict::Foreign;
    }

    FamilyVerdict::Undefined
}
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

    match classify_family(u64::from(family)) {
        FamilyVerdict::Served => {}
        FamilyVerdict::Unsupported => return Err(unsupported("socket.family.unsupported", family)),
        FamilyVerdict::Foreign => return Err(unsupported("socket.family.foreign", family)),
        FamilyVerdict::Undefined => return Err(unsupported("socket.family", family)),
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
    let family = AF_UNIX;
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
