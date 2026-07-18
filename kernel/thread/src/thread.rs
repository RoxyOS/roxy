use core::sync::atomic::{AtomicU64, Ordering};

use roxy_memory::{UserAddress, VirtualAddress};

use crate::{SavedContext, stack::KernelStack};

/// Schedulable execution state with an owned ring-0 stack.
///
/// Every thread needs a kernel stack for context switching and kernel entry. A user thread's user
/// stack is a mapping owned by its address space; only its initial user stack pointer is stored in
/// the saved context.
pub struct Thread {
    id: ThreadId,
    kernel_stack: KernelStack,
    context: SavedContext,
}

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ThreadId(u64);

impl Thread {
    /// Creates a thread with an independent kernel stack.
    ///
    /// # Errors
    ///
    /// Returns an error when the kernel stack cannot be allocated.
    pub fn new(entry: fn() -> !) -> Result<Self, ThreadCreateError> {
        let kernel_stack = KernelStack::new().ok_or(ThreadCreateError::OutOfMemory)?;
        let context = SavedContext::new(&kernel_stack, entry);
        Ok(Self {
            id: ThreadId::new(),
            kernel_stack,
            context,
        })
    }

    /// Creates a ring-3 thread with a kernel-entry stack.
    ///
    /// `user_stack_pointer` identifies the separately mapped user stack. The owned kernel stack is
    /// used only while the thread executes in ring 0 after a context switch, interrupt, or syscall.
    ///
    /// # Errors
    ///
    /// Returns an error when the kernel stack cannot be allocated.
    pub fn new_user(
        user_instruction_pointer: UserAddress,
        user_stack_pointer: UserAddress,
    ) -> Result<Self, ThreadCreateError> {
        let kernel_stack = KernelStack::new().ok_or(ThreadCreateError::OutOfMemory)?;
        let context =
            SavedContext::new_user(&kernel_stack, user_instruction_pointer, user_stack_pointer);
        Ok(Self {
            id: ThreadId::new(),
            kernel_stack,
            context,
        })
    }

    pub fn context(&mut self) -> &mut SavedContext {
        &mut self.context
    }

    #[must_use]
    pub const fn id(&self) -> ThreadId {
        self.id
    }

    /// Returns the exclusive upper bound of the kernel stack.
    ///
    /// # Panics
    ///
    /// Panics if the allocator returned an address outside the architecture's virtual range.
    #[must_use]
    pub fn kernel_stack_top(&self) -> VirtualAddress {
        VirtualAddress::new(u64::try_from(self.kernel_stack.top_address()).unwrap()).unwrap()
    }
}

impl ThreadId {
    fn new() -> Self {
        Self(NEXT_THREAD_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadCreateError {
    OutOfMemory,
}
