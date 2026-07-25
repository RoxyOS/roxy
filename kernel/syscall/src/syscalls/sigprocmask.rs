use crate::{
    SyscallResult, errno::Errno, numbers::SyscallNumber, syscall, unsupported::unsupported_argument,
};

syscall!(SyscallNumber::Sigprocmask, handle(how: u64));

fn handle(how: u64) -> SyscallResult {
    Err(unsupported_argument("sigprocmask", how, Errno::NoSys))
}
