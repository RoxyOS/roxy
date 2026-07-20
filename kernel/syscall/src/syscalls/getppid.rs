use crate::{Syscall, SyscallResult, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Getppid, handle);

#[allow(clippy::unnecessary_wraps)]
fn handle(_arguments: [u64; 6]) -> SyscallResult {
    let parent_process_id = roxy_process::current_parent_process_id();

    Ok(parent_process_id.map_or(0, roxy_process::ProcessId::as_u64))
}
