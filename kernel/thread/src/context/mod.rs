//! Saved kernel contexts used by the scheduler.
//!
//! Saved contexts have stable storage separate from their backing kernel stacks. A user stack is
//! a user-accessible mapping owned by `AddrSpace`; only its initial pointer is carried in the
//! kernel context until the architecture enters ring 3.

#[cfg(target_arch = "x86_64")]
mod x86_64;

use core::sync::atomic::AtomicBool;

use crate::stack::KernelStack;
use roxy_arch::UserContext;
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

    pub(crate) fn new_user_resume(kernel_stack: &KernelStack, context: UserContext) -> Self {
        Self(CurrentContextBackend::new_user_resume(
            kernel_stack,
            context,
        ))
    }

    #[must_use]
    pub fn empty() -> Self {
        Self(CurrentContextBackend::empty())
    }

    /// Switches from `previous` to `next` and returns when `previous` is resumed, and sets
    /// `reserved_ptr` to false.
    ///
    /// # Safety
    ///
    /// `previous` and `next` identify distinct, stable contexts, each reserved for its CPU until
    /// the handoff completes; `next` holds a valid backend-created stack layout. A non-null
    /// `reserved_ptr` points at the outgoing thread's reserved flag and must outlive the handoff
    /// (the caller owns it via the scheduler entry).
    pub unsafe fn switch(previous: *mut Self, next: *const Self, reserved_ptr: *const AtomicBool) {
        // SAFETY: The caller upholds the ownership, lifetime, and stack-layout requirements.
        unsafe {
            CurrentContextBackend::switch(
                core::ptr::addr_of_mut!((*previous).0),
                core::ptr::addr_of!((*next).0),
                reserved_ptr,
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

    fn new_user_resume(kernel_stack: &KernelStack, context: UserContext) -> Self;

    fn empty() -> Self;

    unsafe fn switch(previous: *mut Self, next: *const Self, reserved_ptr: *const AtomicBool);
}
