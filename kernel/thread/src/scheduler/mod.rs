mod control;
mod reap;
mod state;
mod switch;
mod timer_wait;

use roxy_arch::{Architecture, CurrentArchitectureBackend, LocalInterruptKind};
use roxy_utils::Lock;

pub use control::{exit_current, start};
pub use reap::{
    ThreadExitHandler, ThreadReapedHandler, register_exit_handler, register_reaped_handler,
};

use self::state::{Scheduler, ThreadKind};
use crate::{Thread, ThreadCreateError, ThreadId};

static SCHEDULER: Lock<Scheduler> = Lock::new(Scheduler::new());

/// Registers scheduler-owned interrupt consumers.
pub fn initialize() {
    roxy_interrupt::register_local_handler(LocalInterruptKind::Timer, control::on_timer_interrupt);
}

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
    enqueue(thread, ThreadKind::Kernel);
}

pub fn enqueue_user(thread: Thread) {
    enqueue(thread, ThreadKind::User);
}

/// Returns the currently running thread's identifier.
///
/// # Panics
///
/// Panics when called outside a scheduled thread.
pub fn current_thread_id() -> ThreadId {
    SCHEDULER.lock().current_thread_id()
}

/// Registers the hook responsible for activating the next user thread's address space.
///
/// The scheduler invokes the hook with the target thread immediately before switching to it.
///
/// # Panics
///
/// Panics when a hook was already registered.
pub fn register_user_dispatch_hook(hook: fn(ThreadId)) {
    switch::register_user_dispatch_hook(hook);
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

    PendingBlock(SCHEDULER.lock().prepare_block(None))
}

/// Marks the current thread blocked until explicitly woken or the deadline is reached.
///
/// # Panics
///
/// Panics when called outside a scheduled thread, with interrupts enabled, or with an elapsed
/// deadline.
pub fn prepare_block_current_until(deadline: core::time::Duration) -> PendingBlock {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    assert!(
        deadline > roxy_time::monotonic_time(),
        "deadline already elapsed"
    );

    PendingBlock(SCHEDULER.lock().prepare_block(Some(deadline)))
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

fn enqueue(thread: Thread, kind: ThreadKind) {
    CurrentArchitectureBackend::without_interrupts(|| {
        SCHEDULER.lock().enqueue(thread, kind);
    });
}
