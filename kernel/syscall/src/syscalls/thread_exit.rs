use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::ThreadExit, handle());

fn handle() -> SyscallResult {
    roxy_process::thread_exit_current()
}
