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

use roxy_arch::{Architecture, CurrentArchitectureBackend, RawSyscall, SyscallExit};

use crate::{errno::Errno, numbers::SyscallNumber};

type SyscallHandler = fn([u64; 6]) -> SyscallResult;
type ContextualSyscallHandler = fn(RawSyscall) -> SyscallResult;
type SyscallExitHandler = fn(RawSyscall) -> SyscallExit;
type SyscallResult = Result<u64, Errno>;

#[derive(Clone, Copy)]
enum Handler {
    Arguments(SyscallHandler),
    Context(ContextualSyscallHandler),
    /// Returns a `SyscallExit` directly, replacing the syscall-return contract entirely.
    ///
    /// The only handler today is `sigreturn`, which restores an interrupted context instead of
    /// producing a return value; it deliberately skips signal delivery on exit.
    Exit(SyscallExitHandler),
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

    const fn with_exit(number: SyscallNumber, handler: SyscallExitHandler) -> Self {
        Self {
            number,
            handler: Handler::Exit(handler),
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
