use roxy_arch::{RawSyscall, SyscallExit, SyscallOutcome, UserContext};

use crate::{
    Handler,
    errno::Errno,
    numbers::SyscallNumber,
    registry::{REGISTRY, Registry},
};

impl Registry {
    pub(super) fn dispatch(&self, number: SyscallNumber, request: RawSyscall) -> SyscallExit {
        let Some(syscall) = self
            .syscalls
            .iter()
            .find(|syscall| syscall.number == number)
        else {
            crate::unsupported::unsupported_argument("syscall", number as u64, Errno::NoSys);
            return with_pending_signal(failed(Errno::NoSys), &request.context);
        };

        match syscall.handler {
            // An `Exit` handler replaces the syscall-return contract itself, so its `SyscallExit`
            // is used as-is and skips the signal-delivery step on the way out.
            Handler::Exit(handler) => handler(request),
            Handler::Arguments(handler) => {
                syscall_result_to_exit(handler(request.arguments), &request)
            }
            Handler::Context(handler) => syscall_result_to_exit(handler(request), &request),
        }
    }
}

fn syscall_result_to_exit(result: crate::SyscallResult, request: &RawSyscall) -> SyscallExit {
    let outcome = match result {
        Ok(value) => SyscallOutcome::Value(value),
        Err(error) => failed(error),
    };

    with_pending_signal(outcome, &request.context)
}

/// The outcome of a syscall that failed with `error`, carrying no value.
fn failed(error: Errno) -> SyscallOutcome {
    SyscallOutcome::Failed(error.number())
}

/// Wraps a computed outcome into a `SyscallExit`, delivering any pending signal first: a
/// handler turns it into a `Resume`; otherwise the outcome is returned as-is.
fn with_pending_signal(outcome: SyscallOutcome, context: &UserContext) -> SyscallExit {
    // An interrupted blocking syscall returns `EINTR`; that is the only case where a `SA_RESTART`
    // handler should re-execute the syscall after returning. Pass the signal down so delivery can
    // rewind the saved instruction pointer accordingly.
    let is_interrupted = outcome == SyscallOutcome::Failed(Errno::Interrupted.number());

    match roxy_process::deliver_pending_signal(context, is_interrupted) {
        Some(resume) => SyscallExit::Resume { outcome, resume },
        None => SyscallExit::Returned(outcome),
    }
}

pub(super) fn dispatch(request: RawSyscall) -> SyscallExit {
    if let Ok(number) = SyscallNumber::try_from(request.number) {
        REGISTRY.dispatch(number, request)
    } else {
        crate::unsupported::unsupported_argument(
            unknown_syscall(request.number),
            request.number,
            Errno::NoSys,
        );
        with_pending_signal(failed(Errno::NoSys), &request.context)
    }
}

/// Names the diagnostic for a syscall number the table does not resolve.
///
/// A number below [`crate::numbers::SYSCALL_BASE`] carries another personality's numbering, which
/// is what a program with its own syscall layer — a language runtime issuing `syscall` directly
/// with Linux's numbers — ends up sending, and it is reported apart from a number inside the space
/// that this kernel simply does not define. Only the first says the caller was built against a
/// different ABI.
fn unknown_syscall(number: u64) -> &'static str {
    if number < crate::numbers::SYSCALL_BASE {
        "syscall.foreign"
    } else {
        "syscall"
    }
}
