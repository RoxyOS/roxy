use roxy_memory::VirtualAddress;

use crate::{SavedContext, stack::KernelStack};

pub struct Thread {
    stack: KernelStack,
    context: SavedContext,
}

impl Thread {
    /// Creates a thread with an independent kernel stack.
    ///
    /// # Errors
    ///
    /// Returns an error when the kernel stack cannot be allocated.
    pub fn new(entry: fn() -> !) -> Result<Self, ThreadCreateError> {
        let stack = KernelStack::new().ok_or(ThreadCreateError::OutOfMemory)?;
        let context = SavedContext::new(&stack, entry);
        Ok(Self { stack, context })
    }

    pub fn context(&mut self) -> &mut SavedContext {
        &mut self.context
    }

    /// Returns the exclusive upper bound of the kernel stack.
    ///
    /// # Panics
    ///
    /// Panics if the allocator returned an address outside the architecture's virtual range.
    #[must_use]
    pub fn kernel_stack_top(&self) -> VirtualAddress {
        VirtualAddress::new(u64::try_from(self.stack.top_address()).unwrap()).unwrap()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadCreateError {
    OutOfMemory,
}
