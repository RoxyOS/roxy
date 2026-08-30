use roxy_fd::{Fd, ShutdownHow};
use roxy_process::DescriptorError;

use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

impl SyscallArg for ShutdownHow {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        Self::from_raw(raw).ok_or(Errno::Invalid)
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
