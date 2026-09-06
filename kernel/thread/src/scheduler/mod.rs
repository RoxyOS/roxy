mod control;
mod reap;
mod state;
mod switch;

use core::num::NonZeroU64;

use roxy_arch::{Architecture, CurrentArchitectureBackend, LocalInterruptKind};
use roxy_utils::Lock;

pub use control::{allow_ap_dispatch, exit_current, start};
pub use reap::{
    ThreadExitHandler, ThreadReapedHandler, register_exit_handler, register_reaped_handler,
};

use self::state::{Scheduler, ThreadKind, wake_idle_aps};
use crate::{Thread, ThreadCreateError, ThreadId};

static SCHEDULER: Lock<Scheduler> = Lock::new(Scheduler::new());

/// Registers scheduler-owned interrupt consumers and initialises the bootstrap processor's
/// scheduler slot.
pub fn initialize() {
    state::initialize_local();
    roxy_interrupt::register_local_handler(LocalInterruptKind::Timer, control::on_timer_interrupt);
}

/// Initialises the current CPU's scheduler slot without registering the timer handler.
///
/// Each AP calls this once before entering the scheduler control loop. The timer handler is
/// already registered by the bootstrap processor.
pub fn initialize_local() {
    state::initialize_local();
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

/// Returns the current thread identifier when scheduling has started.
#[must_use]
pub fn try_current_thread_id() -> Option<ThreadId> {
    SCHEDULER.lock().try_current_thread_id()
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

/// Uniquely identifies one keyed block registration.
///
/// The key belongs to one block operation rather than one thread. An external wait source must
/// present the same key to wake the thread, preventing a stale notification from an earlier block
/// from waking a later block by that thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WaitKey(NonZeroU64);

impl WaitKey {
    #[must_use]
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }
}

/// Marks the current thread blocked and prepares its context switch.
///
/// # Panics
///
/// Panics when called outside a scheduled thread or with interrupts enabled.
pub fn prepare_block_current() -> PendingBlock {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    PendingBlock(SCHEDULER.lock().prepare_block(None))
}

/// Marks the current thread blocked with a caller-owned wait key.
///
/// # Panics
///
/// Panics when called outside a scheduled thread or with interrupts enabled.
pub fn prepare_block_current_with_key(wait_key: WaitKey) -> PendingBlock {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    PendingBlock(SCHEDULER.lock().prepare_block(Some(wait_key)))
}

impl PendingBlock {
    /// Performs the prepared context switch and returns after the thread is woken.
    pub fn perform(self) {
        self.0.perform();
    }
}

/// Makes a blocked thread runnable regardless of its block state.
///
/// Returns `false` when the thread does not exist or is not blocked.
#[must_use]
pub fn wake_unconditionally(thread_id: ThreadId) -> bool {
    CurrentArchitectureBackend::without_interrupts(|| {
        let woken = SCHEDULER.lock().wake_unconditionally(thread_id);
        if woken {
            wake_idle_aps();
        }
        woken
    })
}

/// Makes a thread runnable only if it is blocked with the supplied wait key.
#[must_use]
pub fn wake_if_waiting(thread_id: ThreadId, wait_key: WaitKey) -> bool {
    CurrentArchitectureBackend::without_interrupts(|| {
        let woken = SCHEDULER.lock().wake_if_waiting(thread_id, wait_key);
        if woken {
            wake_idle_aps();
        }
        woken
    })
}

fn enqueue(thread: Thread, kind: ThreadKind) {
    CurrentArchitectureBackend::without_interrupts(|| {
        SCHEDULER.lock().enqueue(thread, kind);
        wake_idle_aps();
    });
}
