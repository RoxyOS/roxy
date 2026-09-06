use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::GetTid, handle());

#[allow(clippy::unnecessary_wraps)]
fn handle() -> SyscallResult {
    Ok(roxy_thread::scheduler::current_thread_id().as_u64())
}
