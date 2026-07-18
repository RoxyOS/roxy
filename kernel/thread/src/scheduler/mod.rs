mod addrspace;
mod control;
mod reap;
mod state;
mod switch;

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_utils::Lock;
use roxy_vm::AddrSpaceHandle;

pub use control::{exit_current, on_timer_interrupt, start};
pub use reap::{ThreadReapedHandler, register_reaped_handler};

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

fn enqueue(thread: Thread, addrspace: ScheduledAddrSpace) {
    CurrentArchitectureBackend::without_interrupts(|| {
        SCHEDULER.lock().enqueue(thread, addrspace);
    });
}
