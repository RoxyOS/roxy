use roxy_fd::{Fd, IoctlError, IoctlRequest};
use roxy_process::DescriptorError;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Ioctl, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let fd = u32::try_from(arguments[0])
        .map(Fd::new)
        .map_err(|_| Errno::BadFd)?;
    let raw_request = arguments[1];
    let raw_argument = arguments[2];

    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;
    let request = IoctlRequest::parse(raw_request, raw_argument).ok_or(Errno::NotTty)?;

    file.ioctl(request).map_err(map_ioctl_error)
}

fn map_process_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}

fn map_ioctl_error(error: IoctlError) -> Errno {
    match error {
        IoctlError::NotTty => Errno::NotTty,
    }
}
