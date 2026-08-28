use roxy_arch::{RawSyscall, SyscallExit, UserContext};

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
            return with_pending_signal(Errno::NoSys.encode(), &request.context);
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
    match result {
        Ok(value) => with_pending_signal(value, &request.context),
        Err(error) => with_pending_signal(error.encode(), &request.context),
    }
}

/// Wraps a computed return value into a `SyscallExit`, delivering any pending signal first: a
/// handler turns it into a `Resume`; otherwise the value is returned as-is.
fn with_pending_signal(value: u64, context: &UserContext) -> SyscallExit {
    match roxy_process::deliver_pending_signal(context) {
        Some(resume) => SyscallExit::Resume {
            return_value: value,
            resume,
        },
        None => SyscallExit::Returned(value),
    }
}

pub(super) fn dispatch(request: RawSyscall) -> SyscallExit {
    if let Ok(number) = SyscallNumber::try_from(request.number) {
        REGISTRY.dispatch(number, request)
    } else {
        crate::unsupported::unsupported_argument("syscall", request.number, Errno::NoSys);
        with_pending_signal(Errno::NoSys.encode(), &request.context)
    }
}
