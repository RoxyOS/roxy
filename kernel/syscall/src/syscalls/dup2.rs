use bitflags::bitflags;
use roxy_fd::Fd;
use roxy_process::{self, DescriptorError};

use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Dup2, handle(
    oldfd: Fd => BadFd,
    newfd: Fd => BadFd,
    flags: DupFlags => Invalid,
));

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct DupFlags: u64 {
        const CLOEXEC = 0o2_000_000;
    }
}

impl SyscallArg for DupFlags {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        let unknown = raw & !Self::all().bits();
        if unknown != 0 {
            return Err(unsupported("dup2.flags", unknown));
        }
        Ok(Self::from_bits_retain(raw))
    }
}

fn handle(oldfd: Fd, newfd: Fd, flags: DupFlags) -> SyscallResult {
    if oldfd == newfd && !flags.is_empty() {
        return Err(Errno::Invalid); // dup3 语义
    }

    let close_on_exec = flags.contains(DupFlags::CLOEXEC);

    roxy_process::dup2_current(oldfd, newfd, close_on_exec).map_err(map_process_error)?;

    Ok(u64::from(newfd.as_u32()))
}

fn map_process_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}

fn unsupported(operation: &str, argument: u64) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
