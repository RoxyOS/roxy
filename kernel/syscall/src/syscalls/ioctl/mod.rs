mod execute;
mod terminal;
mod terminal_abi;

use roxy_fd::Fd;
use roxy_memory::UserAddress;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

use execute::execute;

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Ioctl, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let fd = u32::try_from(arguments[0])
        .map(Fd::new)
        .map_err(|_| Errno::BadFd)?;
    let raw_request = arguments[1];
    let raw_argument = arguments[2];

    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;
    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;
    let argument = UserAddress::new(raw_argument).ok_or(Errno::Fault)?;

    execute(file.as_ref(), raw_request, argument, &addrspace)?;

    Ok(0)
}
