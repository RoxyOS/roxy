use roxy_fd::{Fd, ShutdownHow};
use roxy_process::DescriptorError;

use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

/// The Roxy `shutdown` `how` values, numbered from a base above Linux's range (its
/// `SHUT_RD`/`SHUT_WR`/`SHUT_RDWR` are 0, 1, 2), so a value below the base is another
/// personality's numbering. See `abi-bits/socket.h`.
const SHUT_RD: u64 = 0x100;
const SHUT_WR: u64 = SHUT_RD << 1;
const SHUT_RDWR: u64 = SHUT_RD << 2;

impl SyscallArg for ShutdownHow {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            SHUT_RD => Ok(Self::Rd),
            SHUT_WR => Ok(Self::Wr),
            SHUT_RDWR => Ok(Self::RdWr),
            value if value < SHUT_RD => Err(super::unsupported("shutdown.how.foreign", value)),
            value => Err(super::unsupported("shutdown.how", value)),
        }
    }
}

syscall!(SyscallNumber::Shutdown, handle(
    fd: Fd => BadFd,
    how: ShutdownHow => Invalid,
));

fn handle(fd: Fd, how: ShutdownHow) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_descriptor_error)?;

    file.socket_ops(|socket| socket.shutdown(how))
        .ok_or(Errno::NotSocket)?
        .map_err(super::map_socket_error)?;

    Ok(0)
}

fn map_descriptor_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}
