mod execute;
mod framebuffer;
mod framebuffer_abi;
mod terminal;
mod terminal_abi;

use roxy_fd::Fd;

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

use execute::execute;

syscall!(SyscallNumber::Ioctl, handle(fd: Fd => BadFd, raw_request: u64, raw_argument: u64));

fn handle(fd: Fd, raw_request: u64, raw_argument: u64) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;

    execute(file.as_ref(), raw_request, raw_argument)?;

    Ok(0)
}
