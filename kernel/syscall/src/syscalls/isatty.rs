use roxy_fd::Fd;
use roxy_process::DescriptorError;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Isatty, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let fd = u32::try_from(arguments[0])
        .map(Fd::new)
        .map_err(|_| Errno::BadFd)?;

    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;

    file.is_terminal().then_some(0).ok_or(Errno::NotTty)
}

fn map_process_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}
