use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Socket, handle(
    domain: Domain => Invalid,
    socket_type: SocketType => Invalid,
    protocol: Protocol => Invalid
));

/// The Roxy socket words follow `abi-bits/socket.h`: each is numbered from a base above its whole
/// Linux range, every member Roxy defines but cannot serve is the family's marker, and a value
/// below the base is another personality's numbering. `AF_INET`, `AF_INET6`, and `SOCK_DGRAM` keep
/// values of their own because an upstream `switch` names them as cases.
const AF_BASE: u64 = 0x100;
const AF_UNSUPPORTED: u64 = 0x80;
/// `AF_INET` and `AF_INET6` keep values of their own because upstream mlibc's `switch` statements
/// name them as cases — collapsing them onto the marker would be a duplicate case label, and that
/// header is not this fork's to edit — but they are served no more than the marker is, so they are
/// reported the same way.
const AF_INET: u64 = AF_BASE + 1;
const AF_INET6: u64 = AF_BASE + 2;

const SOCK_BASE: u64 = 1 << 20;
const SOCK_TYPE_MASK: u64 = (SOCK_BASE << 3) - SOCK_BASE;
const SOCK_UNSUPPORTED: u64 = 1 << 12;
/// `SOCK_DGRAM` takes a value of its own for the same reason `AF_INET` does, and is reported the
/// same way.
const SOCK_DGRAM: u64 = SOCK_BASE + 1;
const SOCK_CLOEXEC: u64 = SOCK_BASE << 4;
const SOCK_NONBLOCK: u64 = SOCK_BASE << 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Domain {
    Unix,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SocketType {
    Stream,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Protocol {
    Default,
}

impl SyscallArg for Domain {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            AF_BASE => Ok(Self::Unix),
            AF_INET | AF_INET6 | AF_UNSUPPORTED => {
                Err(unsupported("socket.domain.unsupported", raw))
            }
            value if value < AF_BASE => Err(unsupported("socket.domain.foreign", value)),
            value => Err(unsupported("socket.domain", value)),
        }
    }
}

impl SyscallArg for SocketType {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        if raw == SOCK_UNSUPPORTED || raw == SOCK_DGRAM {
            return Err(unsupported("socket.type.unsupported", raw));
        }

        // Descriptor flags are rejected with `EINVAL` rather than `ENOTSUP` because callers
        // such as libxcb retry without them exactly when `socket()` fails with `EINVAL`.
        if raw & (SOCK_CLOEXEC | SOCK_NONBLOCK) != 0 {
            return Err(unsupported("socket.descriptor-flags", raw));
        }

        let foreign = raw & (SOCK_BASE - 1);

        if foreign != 0 {
            return Err(unsupported("socket.type.foreign", foreign));
        }

        match raw & SOCK_TYPE_MASK {
            SOCK_BASE => Ok(Self::Stream),
            _ => Err(unsupported("socket.type", raw)),
        }
    }
}

impl SyscallArg for Protocol {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            0 => Ok(Self::Default),
            _ => Err(unsupported("socket.protocol", raw)),
        }
    }
}

/// Creates one unconnected socket and inserts it into the caller's descriptor table.
///
/// Argument parsing rejects every unsupported combination, so the implementation itself cannot
/// fail.
#[allow(clippy::unnecessary_wraps)]
fn handle(domain: Domain, socket_type: SocketType, protocol: Protocol) -> SyscallResult {
    let socket = match (domain, socket_type, protocol) {
        (Domain::Unix, SocketType::Stream, Protocol::Default) => roxy_unix_socket::stream::socket(),
    };

    let fd = roxy_process::insert_open_file(socket, false);

    Ok(u64::from(fd.as_u32()))
}

fn unsupported(operation: &str, argument: u64) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::Invalid)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{Domain, Protocol, SocketType};
    use crate::args::SyscallArg;
    use crate::errno::Errno;

    kernel_test!(
        "roxy-syscall::socket-arguments",
        parses_supported_arguments,
        {
            assert_eq!(Domain::parse(1, Errno::Invalid), Ok(Domain::Unix));
            assert_eq!(SocketType::parse(1, Errno::Invalid), Ok(SocketType::Stream));
            assert_eq!(Protocol::parse(0, Errno::Invalid), Ok(Protocol::Default));
        }
    );

    kernel_test!(
        "roxy-syscall::socket-arguments",
        rejects_unsupported_arguments,
        {
            assert_eq!(Domain::parse(2, Errno::Invalid), Err(Errno::Invalid));
            assert_eq!(Protocol::parse(6, Errno::Invalid), Err(Errno::Invalid));
            assert_eq!(SocketType::parse(2, Errno::Invalid), Err(Errno::Invalid));
        }
    );

    kernel_test!(
        "roxy-syscall::socket-arguments",
        rejects_descriptor_flags,
        {
            let cloexec = 1 | 0o2_000_000;
            let nonblocking = 1 | 0o4000;
            let unknown_flag = 1 | (1 << 20);

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
