use crate::{
    SyscallResult, errno::Errno, numbers::SyscallNumber, syscall, unsupported::unsupported_argument,
};

syscall!(SyscallNumber::Sigaction, handle(signal: u64));

fn handle(signal: u64) -> SyscallResult {
    Err(unsupported_argument("sigaction", signal, Errno::NoSys))
}
