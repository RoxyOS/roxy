use crate::{
    Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber, unsupported::unsupported_argument,
};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Sigprocmask, handle);

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let how = arguments[0];

    Err(unsupported_argument("sigprocmask", how, Errno::NoSys))
}
