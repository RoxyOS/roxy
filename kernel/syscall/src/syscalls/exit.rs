use roxy_process::ExitStatus;

use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Exit, handle(status: u64));

fn handle(status: u64) -> SyscallResult {
    roxy_process::exit_current(ExitStatus::new(status))
}
