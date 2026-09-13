use super::{Domain, Protocol, SocketType};
use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Socket, handle(
    domain: Domain => Invalid,
    socket_type: SocketType => Invalid,
    protocol: Protocol => Invalid
));

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
