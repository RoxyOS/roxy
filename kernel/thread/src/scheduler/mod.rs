mod addrspace;
mod control;
mod reap;
mod state;
mod switch;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_utils::Lock;
use roxy_vm::AddrSpaceHandle;

pub use control::{exit_current, on_timer_interrupt, start};
pub use reap::{
    ThreadExitHandler, ThreadReapedHandler, register_exit_handler, register_reaped_handler,
};

use self::{addrspace::ScheduledAddrSpace, state::Scheduler};
use crate::{Thread, ThreadCreateError, ThreadId};

static SCHEDULER: Lock<Scheduler> = Lock::new(Scheduler::new());

/// Creates and enqueues a permanently runnable kernel thread.
///
/// # Errors
///
/// Returns an error when its kernel stack cannot be allocated.
pub fn spawn(entry: fn() -> !) -> Result<(), ThreadCreateError> {
    let thread = Thread::new(entry)?;
    enqueue_kernel(thread);
    Ok(())
}

pub fn enqueue_kernel(thread: Thread) {
    enqueue(thread, ScheduledAddrSpace::Kernel);
}

pub fn enqueue_user(thread: Thread, addrspace: AddrSpaceHandle) {
    enqueue(thread, ScheduledAddrSpace::User(addrspace));
}

/// Returns the currently running thread's identifier.
///
/// # Panics
///
/// Panics when called outside a scheduled thread.
pub fn current_thread_id() -> ThreadId {
    SCHEDULER.lock().current_thread_id()
}

#[must_use = "a prepared block must be performed"]
pub struct PendingBlock(switch::PendingContextSwitch);

/// Marks the current thread blocked and prepares its context switch.
///
/// # Panics
///
/// Panics when called outside a scheduled thread or with interrupts enabled.
pub fn prepare_block_current() -> PendingBlock {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    PendingBlock(SCHEDULER.lock().prepare_block())
}

impl PendingBlock {
    /// Performs the prepared context switch and returns after the thread is woken.
    pub fn perform(self) {
        self.0.perform();
    }
}

/// Makes a blocked thread runnable.
///
/// Returns `false` when the thread does not exist or is not blocked.
#[must_use]
pub fn wake(thread_id: ThreadId) -> bool {
    CurrentArchitectureBackend::without_interrupts(|| SCHEDULER.lock().wake(thread_id))
}

fn enqueue(thread: Thread, addrspace: ScheduledAddrSpace) {
    CurrentArchitectureBackend::without_interrupts(|| {
        SCHEDULER.lock().enqueue(thread, addrspace);
    });
}
