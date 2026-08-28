use roxy_unix_socket::stream;

use crate::{
    SyscallResult,
    args::{Out, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::Socketpair, handle(
    domain: Domain => Invalid,
    socket_type: SocketType => Invalid,
    protocol: Protocol => Invalid,
    output: Out<[i32; 2]> => Fault
));

#[derive(Clone, Copy)]
enum Domain {
    Unix,
}

#[derive(Clone, Copy)]
enum SocketType {
    Stream,
}

#[derive(Clone, Copy)]
enum Protocol {
    Default,
}

impl SyscallArg for Domain {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            1 => Ok(Self::Unix),
            _ => Err(unsupported("socketpair.domain", raw)),
        }
    }
}

impl SyscallArg for SocketType {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            1 => Ok(Self::Stream),
            _ => Err(unsupported("socketpair.type", raw)),
        }
    }
}

impl SyscallArg for Protocol {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            0 => Ok(Self::Default),
            _ => Err(unsupported("socketpair.protocol", raw)),
        }
    }
}

fn handle(
    domain: Domain,
    socket_type: SocketType,
    protocol: Protocol,
    output: Out<[i32; 2]>,
) -> SyscallResult {
    output.validate()?;

    let (first, second) = match (domain, socket_type, protocol) {
        (Domain::Unix, SocketType::Stream, Protocol::Default) => stream::pair(),
    };

    let first_fd = roxy_process::insert_open_file(first);
    let second_fd = roxy_process::insert_open_file(second);

    let descriptors = if let (Ok(first), Ok(second)) = (
        i32::try_from(first_fd.as_u32()),
        i32::try_from(second_fd.as_u32()),
    ) {
        [first, second]
    } else {
        close_pair(first_fd, second_fd);

        return Err(Errno::Overflow);
    };

    // SAFETY: The array contains two initialized i32 values and has no padding.
    if let Err(error) = unsafe { output.write(&descriptors) } {
        close_pair(first_fd, second_fd);

        return Err(error);
    }

    Ok(0)
}

fn close_pair(first: roxy_fd::Fd, second: roxy_fd::Fd) {
    roxy_process::close_file(first).expect("new socket descriptor must remain open");
    roxy_process::close_file(second).expect("new socket descriptor must remain open");
}

fn unsupported(operation: &str, argument: u64) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
