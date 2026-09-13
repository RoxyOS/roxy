use super::{FamilyVerdict, classify_family};
use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Socket, handle(
    domain: Domain => Invalid,
    socket_type: SocketType => Invalid,
    protocol: Protocol => Invalid
));

/// The Roxy socket-type word follows `abi-bits/socket.h`: `SOCK_STREAM` is the first value of a
/// three-bit type field at the base, the two supported flags sit above the field, and a value below
/// the base is another personality's numbering. `SOCK_DGRAM` keeps a value of its own because
/// upstream mlibc's `switch` names it as a case, and is reported as unsupported all the same.
const SOCK_BASE: u64 = 1 << 20;
const SOCK_TYPE_MASK: u64 = (SOCK_BASE << 3) - SOCK_BASE;
const SOCK_UNSUPPORTED: u64 = 1 << 12;
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
        match classify_family(raw) {
            FamilyVerdict::Served => Ok(Self::Unix),
            FamilyVerdict::Unsupported => Err(unsupported("socket.domain.unsupported", raw)),
            FamilyVerdict::Foreign => Err(unsupported("socket.domain.foreign", raw)),
            FamilyVerdict::Undefined => Err(unsupported("socket.domain", raw)),
        }
    }
}

impl SyscallArg for SocketType {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        // The marker is a bit, so a caller that ORs it into a supported value is still recognised
        // as asking for something Roxy cannot serve rather than as passing a foreign number.
        if raw & SOCK_UNSUPPORTED != 0 || raw == SOCK_DGRAM {
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

    kernel_test!("roxy-syscall::socket-family", classifies_the_family_word, {
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
    });

    kernel_test!(
        "roxy-syscall::socket-arguments",
        rejects_unsupported_arguments,
        {
            // Anything below the bases is another personality's numbering: Linux spells the Unix
            // family 1 and the stream type 1.
            assert_eq!(Domain::parse(1, Errno::Invalid), Err(Errno::Invalid));
            assert_eq!(SocketType::parse(1, Errno::Invalid), Err(Errno::Invalid));
            assert_eq!(Protocol::parse(6, Errno::Invalid), Err(Errno::Invalid));

            // The marker, and the three names that keep values of their own, are what the header
            // offers a caller who wants something Roxy does not serve; each is refused whether it
            // arrives alone or ORed into a supported value.
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
