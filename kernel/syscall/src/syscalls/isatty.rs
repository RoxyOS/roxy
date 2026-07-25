use roxy_fd::Fd;
use roxy_process::DescriptorError;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Isatty, handle(fd: Fd => BadFd));

fn handle(fd: Fd) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;

    file.is_terminal().then_some(0).ok_or(Errno::NotTty)
}

fn map_process_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}
