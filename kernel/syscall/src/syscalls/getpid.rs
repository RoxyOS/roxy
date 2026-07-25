use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Getpid, handle());

#[allow(clippy::unnecessary_wraps)]
fn handle() -> SyscallResult {
    Ok(roxy_process::current_process_id().as_u64())
}
