#[cfg(target_arch = "x86_64")]
mod x86_64;

use crate::stack::KernelStack;

#[cfg(target_arch = "x86_64")]
use self::x86_64::X86_64Context;

#[cfg(target_arch = "x86_64")]
type CurrentContextBackend = X86_64Context;

pub struct SavedContext(CurrentContextBackend);

impl SavedContext {
    pub(crate) fn new(stack: &KernelStack, entry: fn() -> !) -> Self {
        Self(CurrentContextBackend::new(stack, entry))
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
    fn new(stack: &KernelStack, entry: fn() -> !) -> Self;

    fn empty() -> Self;

    unsafe fn switch(previous: *mut Self, next: *const Self);
}
