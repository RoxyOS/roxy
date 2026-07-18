use roxy_abi::SyscallNumber;
use roxy_process::ExitStatus;

use crate::{Syscall, SyscallResult};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Exit, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    roxy_process::exit_current(ExitStatus(arguments[0]))
}
