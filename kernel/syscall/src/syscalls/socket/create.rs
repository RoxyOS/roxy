use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Socket, handle(
    domain: Domain => Invalid,
    socket_type: SocketType => Invalid,
    protocol: Protocol => Invalid
));

const TYPE_MASK: u64 = 0xf;
const TYPE_STREAM: u64 = 1;
const FLAG_CLOEXEC: u64 = 0o2_000_000;
const FLAG_NONBLOCK: u64 = 0o4000;

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
            1 => Ok(Self::Unix),
            _ => Err(unsupported("socket.domain", raw)),
        }
    }
}

impl SyscallArg for SocketType {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        if raw & !(TYPE_MASK | FLAG_CLOEXEC | FLAG_NONBLOCK) != 0 {
            return Err(unsupported("socket.flags", raw));
        }

        // Descriptor flags are rejected with `EINVAL` rather than `ENOTSUP` because callers
        // such as libxcb retry without them exactly when `socket()` fails with `EINVAL`.
        if raw & (FLAG_CLOEXEC | FLAG_NONBLOCK) != 0 {
            return Err(unsupported("socket.descriptor-flags", raw));
        }

        match raw & TYPE_MASK {
            TYPE_STREAM => Ok(Self::Stream),
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

    let fd = roxy_process::insert_open_file(socket);

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
