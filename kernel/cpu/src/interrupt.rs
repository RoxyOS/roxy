use core::sync::atomic::Ordering;

use roxy_arch::{Architecture, CpuId, CurrentArchitectureBackend, LocalInterruptKind};

use crate::{
    arch::{CpuBackend, CurrentCpuBackend},
    cpu::CPU_STATE,
    timer,
};

pub fn handle_local_interrupt(kind: LocalInterruptKind) {
    let _guard = InterruptGuard::new();

    match kind {
        LocalInterruptKind::Timer => handle_timer(),
        LocalInterruptKind::Error => handle_error(),
        LocalInterruptKind::Spurious => handle_spurious(),
    }
}

fn handle_timer() {
    if CurrentArchitectureBackend::current_cpu_id() == CpuId::BSP {
        timer::advance_time();
    }
    CPU_STATE.get().timer_ticks.fetch_add(1, Ordering::Relaxed);
    CurrentCpuBackend::end_of_interrupt();
}

fn handle_error() {
    let flags = CurrentCpuBackend::error_flags();
    CPU_STATE
        .get()
        .last_apic_error
        .store(flags, Ordering::Relaxed);
    CPU_STATE.get().apic_errors.fetch_add(1, Ordering::Relaxed);
    CurrentCpuBackend::end_of_interrupt();
}

fn handle_spurious() {
    CPU_STATE
        .get()
        .spurious_interrupts
        .fetch_add(1, Ordering::Relaxed);
}

/// RAII guard that tracks the current CPU's interrupt nesting depth.
struct InterruptGuard;

impl InterruptGuard {
    fn new() -> Self {
        let state = CPU_STATE.get();
        state
            .interrupt_depth
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |depth| {
                depth.checked_add(1)
            })
            .expect("interrupt nesting depth overflow");
        state.interrupt_entries.fetch_add(1, Ordering::Relaxed);
        Self
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        CPU_STATE
            .get()
            .interrupt_depth
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |depth| {
                depth.checked_sub(1)
            })
            .expect("unbalanced interrupt exit");
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::sync::atomic::Ordering;

    use super::InterruptGuard;
    use crate::cpu::CPU_STATE;

    roxy_test::kernel_test!("roxy-cpu::interrupt-nesting-restores", irq_nesting, {
        assert_eq!(CPU_STATE.get().interrupt_depth.load(Ordering::Relaxed), 0);
        {
            let _outer = InterruptGuard::new();
            assert_eq!(CPU_STATE.get().interrupt_depth.load(Ordering::Relaxed), 1);
            {
                let _inner = InterruptGuard::new();
                assert_eq!(CPU_STATE.get().interrupt_depth.load(Ordering::Relaxed), 2);
            }
            assert_eq!(CPU_STATE.get().interrupt_depth.load(Ordering::Relaxed), 1);
        }
        assert_eq!(CPU_STATE.get().interrupt_depth.load(Ordering::Relaxed), 0);
    });
}
