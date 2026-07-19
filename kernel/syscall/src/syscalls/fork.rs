use roxy_arch::RawSyscall;
use roxy_process::ForkError;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::with_context(SyscallNumber::Fork, handle);

fn handle(request: RawSyscall) -> SyscallResult {
    let child = roxy_process::fork_current(request.context).map_err(map_fork_error)?;

    Ok(child.as_u64())
}

fn map_fork_error(error: ForkError) -> Errno {
    match error {
        ForkError::OutOfMemory => Errno::NoMem,
        ForkError::InvalidAddressSpace => Errno::Fault,
    }
}
