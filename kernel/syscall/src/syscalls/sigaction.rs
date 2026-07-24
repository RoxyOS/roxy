use crate::{
    Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber, unsupported::unsupported_argument,
};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Sigaction, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let signal = arguments[0];

    Err(unsupported_argument("sigaction", signal, Errno::NoSys))
}
