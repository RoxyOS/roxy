use roxy_signal::Signal;

use crate::{
    SyscallResult, errno::Errno, numbers::SyscallNumber, syscall, unsupported::unsupported_argument,
};

syscall!(SyscallNumber::Sigaction, handle(signal: Signal => Invalid));

fn handle(signal: Signal) -> SyscallResult {
    Err(unsupported_argument(
        "sigaction",
        signal.number(),
        Errno::NoSys,
    ))
}
