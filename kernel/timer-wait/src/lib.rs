#![no_std]

extern crate alloc;

mod queue;

use core::time::Duration;

use roxy_arch::{Architecture, CurrentArchitectureBackend, LocalInterruptKind};
use roxy_thread::scheduler::PendingBlock;

use crate::queue::TIMER_WAITERS;

/// Registers the deadline queue with periodic timer delivery.
pub fn initialize() {
    roxy_interrupt::register_local_handler(LocalInterruptKind::Timer, on_timer_interrupt);
}

/// Registers the current thread for `deadline` and prepares its context switch.
///
/// # Panics
///
/// Panics when interrupts are enabled or the deadline has already elapsed.
pub fn block_current(deadline: Duration) -> PendingBlock {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    assert!(
        deadline > roxy_time::monotonic_time(),
        "deadline already elapsed"
    );

    let thread_id = roxy_thread::scheduler::current_thread_id();
    let wait_key = TIMER_WAITERS.lock().register(thread_id, deadline).key();

    roxy_thread::scheduler::prepare_block_current_with_key(wait_key)
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
