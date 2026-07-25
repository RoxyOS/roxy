use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Getppid, handle());

#[allow(clippy::unnecessary_wraps)]
fn handle() -> SyscallResult {
    let parent_process_id = roxy_process::current_parent_process_id();

    Ok(parent_process_id.map_or(0, roxy_process::ProcessId::as_u64))
}
