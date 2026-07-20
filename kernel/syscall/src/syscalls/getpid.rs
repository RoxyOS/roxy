use crate::{Syscall, SyscallResult, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Getpid, handle);

fn handle(_arguments: [u64; 6]) -> SyscallResult {
    Ok(roxy_process::current_process_id().as_u64())
}
