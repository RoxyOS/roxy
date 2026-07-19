use roxy_fd::Fd;
use roxy_process::DescriptorError;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Close, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let fd = u32::try_from(arguments[0])
        .map(Fd::new)
        .map_err(|_| Errno::BadFd)?;

    roxy_process::close_file(fd).map_err(map_process_error)?;

    Ok(0)
}

fn map_process_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}
