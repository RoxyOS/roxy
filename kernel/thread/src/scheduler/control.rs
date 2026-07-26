use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_utils::preemption;

use super::{
    SCHEDULER,
    reap::{notify_exit, notify_reaped},
};

pub fn start() -> ! {
    CurrentArchitectureBackend::without_interrupts(|| {
        loop {
            let prepared = SCHEDULER.lock().prepare_dispatch();
            notify_reaped(prepared.reaped);
            notify_exit(prepared.exiting);

            if let Some(pending_switch) = prepared.pending_switch {
                pending_switch.perform();
            } else {
                CurrentArchitectureBackend::wait_for_interrupt();
            }
        }
    })
}

/// Exits the current thread without releasing its active kernel stack in place.
///
/// # Panics
///
/// Panics when called outside a scheduled thread or with interrupts enabled.
pub fn exit_current() -> ! {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    let prepared = SCHEDULER.lock().prepare_exit();
    notify_reaped(prepared.reaped);
    notify_exit(prepared.exiting);
    prepared.pending_switch.unwrap().perform();
    panic!("exited thread resumed")
}

/// Applies timer-driven round-robin preemption.
///
/// # Panics
///
/// Panics when called with interrupts enabled.
pub(super) fn on_timer_interrupt() {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    SCHEDULER.lock().wake_expired(roxy_time::monotonic_time());

    if preemption::is_disabled() {
        return;
    }

    let prepared = SCHEDULER.lock().prepare_preemption();
    notify_reaped(prepared.reaped);

    if let Some(pending_switch) = prepared.pending_switch {
        pending_switch.perform();
    }
}
