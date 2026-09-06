use core::sync::atomic::{AtomicBool, Ordering};

use roxy_arch::{Architecture, CpuId, CurrentArchitectureBackend};
use roxy_utils::preemption;

use super::{
    SCHEDULER,
    reap::{notify_exit, notify_reaped},
};

/// True once the bootstrap processor has finished boot and wants application processors to take
/// threads from the shared run queue.
///
/// APs only start dispatching after this is set (by `allow_ap_dispatch`), so they never steal the
/// still-booting thread or during the fragile startup window before the initial process is ready.
static APS_READY: AtomicBool = AtomicBool::new(false);

/// Lets application processors begin dispatching threads from the shared run queue.
///
/// The bootstrap processor calls this once after spawning the initial process and wiring its
/// terminal, immediately before entering [`start`].
pub fn allow_ap_dispatch() {
    APS_READY.store(true, Ordering::Release);
}

pub fn start() -> ! {
    CurrentArchitectureBackend::without_interrupts(|| {
        // The bootstrap processor (cpu id 0) is always cleared to dispatch its boot thread; only
        // application processors wait for the readiness signal.
        let is_bootstrap = is_bootstrap_processor();

        loop {
            if !is_bootstrap && !APS_READY.load(Ordering::Acquire) {
                CurrentArchitectureBackend::wait_for_interrupt();
                continue;
            }

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

/// Whether the current CPU is the bootstrap processor.
fn is_bootstrap_processor() -> bool {
    CurrentArchitectureBackend::current_cpu_id() == CpuId::BSP
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
///
/// Runs on whichever CPU's timer fired. The handler is registered once in the global interrupt
/// registry, so every CPU's LAPIC timer invokes this same function pointer on the interrupting
/// CPU. Per-CPU behaviour comes from reading per-CPU state (preemption depth, `local().current`)
/// rather than from a per-CPU registration.
pub(super) fn on_timer_interrupt() {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());

    if preemption::is_disabled() {
        return;
    }

    let prepared = SCHEDULER.lock().prepare_preemption();
    notify_reaped(prepared.reaped);

    if let Some(pending_switch) = prepared.pending_switch {
        pending_switch.perform();
    }
}
