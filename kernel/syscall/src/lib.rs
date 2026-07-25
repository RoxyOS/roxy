#![no_std]

extern crate alloc;

pub(crate) mod args;
mod dispatch;
pub(crate) use args::syscall;
mod errno;
mod numbers;
mod registry;
mod syscalls;
mod unsupported;

use roxy_arch::{Architecture, CurrentArchitectureBackend, RawSyscall};

use crate::{errno::Errno, numbers::SyscallNumber};

type SyscallHandler = fn([u64; 6]) -> SyscallResult;
type ContextualSyscallHandler = fn(RawSyscall) -> SyscallResult;
type SyscallResult = Result<u64, Errno>;

#[derive(Clone, Copy)]
enum Handler {
    Arguments(SyscallHandler),
    Context(ContextualSyscallHandler),
}

struct Syscall {
    number: SyscallNumber,
    handler: Handler,
}

impl Syscall {
    const fn new(number: SyscallNumber, handler: SyscallHandler) -> Self {
        Self {
            number,
            handler: Handler::Arguments(handler),
        }
    }

    const fn with_context(number: SyscallNumber, handler: ContextualSyscallHandler) -> Self {
        Self {
            number,
            handler: Handler::Context(handler),
        }
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
