use roxy_fd::Fd;
use roxy_process::DescriptorError;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Close, handle(fd: Fd => BadFd));

fn handle(fd: Fd) -> SyscallResult {
    roxy_process::close_file(fd).map_err(map_process_error)?;

    Ok(0)
}

fn map_process_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}
