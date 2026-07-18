#![no_std]

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_process::ExitStatus;

const EXIT_SYSCALL: u64 = 0;

pub fn initialize() {
    CurrentArchitectureBackend::configure_syscall(dispatch);
}

fn dispatch(number: u64, argument: u64) -> ! {
    assert_eq!(number, EXIT_SYSCALL, "unknown syscall {number}");
    roxy_process::exit_current(ExitStatus(argument))
}
