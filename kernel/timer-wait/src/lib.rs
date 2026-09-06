#![no_std]

extern crate alloc;

mod queue;

use core::time::Duration;

use roxy_arch::{Architecture, CurrentArchitectureBackend, LocalInterruptKind};
use roxy_thread::scheduler::{PendingBlock, WaitKey};

use crate::queue::TIMER_WAITERS;

/// Registers the deadline queue with periodic timer delivery.
pub fn initialize() {
    roxy_interrupt::register_local_handler(LocalInterruptKind::Timer, on_timer_interrupt);
}

/// Registers the current thread for `deadline` and prepares its context switch.
///
/// # Panics
///
/// Panics when interrupts are enabled.
///
/// A deadline that has already elapsed is not an error: it is registered as-is and only adds up
/// to one timer tick (4 ms) of latency before [`register_wakeup_deadline`]'s wakeup fires.
pub fn block_current(deadline: Duration) -> PendingBlock {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    let wait_key = TIMER_WAITERS.lock().next_key();
    register_wakeup_deadline(deadline, wait_key);

    roxy_thread::scheduler::prepare_block_current_with_key(wait_key)
}

/// Registers a deadline that wakes the current thread with `wait_key`.
///
/// This only registers a wakeup source. It does not block, change scheduler state, or perform a
/// context switch. The caller must prepare and perform the block separately.
///
/// # Panics
///
/// Panics when interrupts are enabled.
///
/// A deadline that has already elapsed is registered anyway: the next timer tick removes it via
/// [`TIMER_WAITERS`] and wakes the current thread immediately, so the caller never blocks past the
/// deadline even when the clock crossed it after the deadline snapshot was taken.
pub fn register_wakeup_deadline(deadline: Duration, wait_key: WaitKey) {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    let thread_id = roxy_thread::scheduler::current_thread_id();
    TIMER_WAITERS.lock().register(thread_id, deadline, wait_key);
}

/// Removes the wakeup deadline registered with `wait_key`, if it has not expired.
///
/// This only cancels the wakeup source. It does not wake, block, or change the current thread's
/// scheduler state.
///
/// # Panics
///
/// Panics when interrupts are enabled.
pub fn cancel_wakeup_deadline(wait_key: WaitKey) {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    TIMER_WAITERS.lock().cancel(wait_key);
}

fn on_timer_interrupt() {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    while let Some(waiter) = TIMER_WAITERS
        .lock()
        .take_expired(roxy_time::monotonic_time())
    {
        let _ = roxy_thread::scheduler::wake_if_waiting(waiter.thread_id, waiter.key());
    }
}
