#![no_std]

mod dispatch;
mod registry;
mod syscalls;

use roxy_abi::{Errno, SyscallNumber};
use roxy_arch::{Architecture, CurrentArchitectureBackend};

type SyscallHandler = fn([u64; 6]) -> SyscallResult;
type SyscallResult = Result<u64, Errno>;

struct Syscall {
    number: SyscallNumber,
    handler: SyscallHandler,
}

impl Syscall {
    const fn new(number: SyscallNumber, handler: SyscallHandler) -> Self {
        Self { number, handler }
    }
}

/// Validates the syscall registry and configures the architecture entry point.
///
/// # Panics
///
/// Panics when the registry contains duplicate syscall numbers or the architecture entry was
/// already configured.
pub fn initialize() {
    assert_eq!(
        registry::REGISTRY.validate(),
        Ok(()),
        "invalid syscall registry"
    );
    CurrentArchitectureBackend::configure_syscall(dispatch::dispatch);
}
