#![no_std]

#[cfg(target_arch = "x86_64")]
mod x86_64;

use roxy_memory::VirtualAddress;

/// Installs the temporary exit-only syscall entry for one user thread.
///
/// The registered kernel stack must belong to the user thread that will run next. This temporary
/// single-thread contract is replaced when CPU-local syscall state is introduced.
pub fn initialize(kernel_stack_top: VirtualAddress) {
    // SAFETY: the x86_64 entry is permanent and the caller retains the thread's kernel stack.
    unsafe { x86_64::initialize(kernel_stack_top) };
}
