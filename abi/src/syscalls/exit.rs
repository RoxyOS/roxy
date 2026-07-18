use crate::{
    SyscallNumber,
    arch::{ArchitectureBackend, CurrentArchitectureBackend},
};

/// Invokes the Roxy exit syscall and never returns.
#[unsafe(no_mangle)]
pub extern "C" fn roxy_syscall_exit(status: u64) -> ! {
    // SAFETY: Exit accepts one scalar argument in the first argument register and never returns.
    unsafe { CurrentArchitectureBackend::syscall1_noreturn(SyscallNumber::Exit as u64, status) }
}
