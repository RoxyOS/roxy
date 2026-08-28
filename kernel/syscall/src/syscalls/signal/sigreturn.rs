use roxy_arch::{RawSyscall, SyscallExit};

use crate::{Syscall, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::with_exit(SyscallNumber::Sigreturn, handle);

/// Restores the interrupted user context from the most recent signal frame, replacing the
/// syscall-return contract entirely.
///
/// Returns `EINVAL` when no frame matches the caller's stack pointer, which is the kernel's
/// guard against a forged or unbalanced frame.
fn handle(request: RawSyscall) -> SyscallExit {
    match roxy_process::pop_signal_frame(&request.context) {
        Some(restored) => SyscallExit::RestoreContext(restored),
        None => SyscallExit::Returned(Errno::Invalid.encode()),
    }
}
