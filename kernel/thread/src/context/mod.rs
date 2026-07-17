//! Saved kernel contexts used by the scheduler.
//!
//! Every [`SavedContext`] is stored on a thread-owned [`KernelStack`]. A user stack is different:
//! it is a user-accessible mapping owned by `AddrSpace`, and only its initial pointer is carried in
//! the kernel context until the architecture enters ring 3.

#[cfg(target_arch = "x86_64")]
mod x86_64;

use crate::stack::KernelStack;
use roxy_memory::UserAddress;

#[cfg(target_arch = "x86_64")]
use self::x86_64::X86_64Context;

#[cfg(target_arch = "x86_64")]
type CurrentContextBackend = X86_64Context;

pub struct SavedContext(CurrentContextBackend);

impl SavedContext {
    pub(crate) fn new(kernel_stack: &KernelStack, entry: fn() -> !) -> Self {
        Self(CurrentContextBackend::new_kernel(kernel_stack, entry))
    }

    pub(crate) fn new_user(
        kernel_stack: &KernelStack,
        user_instruction_pointer: UserAddress,
        user_stack_pointer: UserAddress,
    ) -> Self {
        Self(CurrentContextBackend::new_user(
            kernel_stack,
            user_instruction_pointer,
            user_stack_pointer,
        ))
    }

    #[must_use]
    pub fn empty() -> Self {
        Self(CurrentContextBackend::empty())
    }

    /// Switches from `previous` to `next` and returns when `previous` is resumed.
    ///
    /// # Safety
    ///
    /// Both pointers must identify distinct, exclusively owned contexts whose backing stacks stay
    /// alive for the entire suspension. `next` must contain a valid backend-created stack layout.
    pub unsafe fn switch(previous: *mut Self, next: *const Self) {
        // SAFETY: The caller upholds the ownership, lifetime, and stack-layout requirements.
        unsafe {
            CurrentContextBackend::switch(
                core::ptr::addr_of_mut!((*previous).0),
                core::ptr::addr_of!((*next).0),
            );
        }
    }
}

trait ContextBackend: Sized {
    fn new_kernel(kernel_stack: &KernelStack, entry: fn() -> !) -> Self;

    fn new_user(
        kernel_stack: &KernelStack,
        user_instruction_pointer: UserAddress,
        user_stack_pointer: UserAddress,
    ) -> Self;

    fn empty() -> Self;

    unsafe fn switch(previous: *mut Self, next: *const Self);
}
