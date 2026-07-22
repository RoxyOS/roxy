use core::sync::atomic::Ordering;

use roxy_arch::{Interrupt, IrqLine, LocalInterruptKind};

use crate::{
    arch::{CurrentInterruptBackend, InterruptBackend},
    registry,
    state::{self, INTERRUPT_STATE},
};

pub(crate) fn handle(interrupt: Interrupt) {
    match interrupt {
        Interrupt::Local(kind) => dispatch_local(kind),
        Interrupt::Irq(line) => dispatch_irq(line),
    }
}

fn dispatch_local(kind: LocalInterruptKind) {
    {
        let _guard = InterruptGuard::new();
        if requires_eoi(kind) {
            CurrentInterruptBackend::end_of_interrupt();
        }
    }

    registry::notify_local(kind);
}

fn dispatch_irq(line: IrqLine) {
    let _guard = InterruptGuard::new();
    registry::notify_irq(line);
    state::record_irq(line);
    CurrentInterruptBackend::end_of_interrupt();
}

const fn requires_eoi(kind: LocalInterruptKind) -> bool {
    !matches!(kind, LocalInterruptKind::Spurious)
}

struct InterruptGuard;

impl InterruptGuard {
    fn new() -> Self {
        let state = INTERRUPT_STATE.get();
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
        INTERRUPT_STATE
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

    use roxy_arch::LocalInterruptKind;

    use super::{InterruptGuard, requires_eoi};
    use crate::state::INTERRUPT_STATE;

    roxy_test::kernel_test!("roxy-interrupt::interrupt-nesting-restores", irq_nesting, {
        assert_eq!(
            INTERRUPT_STATE
                .get()
                .interrupt_depth
                .load(Ordering::Relaxed),
            0
        );
        {
            let _outer = InterruptGuard::new();
            assert_eq!(
                INTERRUPT_STATE
                    .get()
                    .interrupt_depth
                    .load(Ordering::Relaxed),
                1
            );
            {
                let _inner = InterruptGuard::new();
                assert_eq!(
                    INTERRUPT_STATE
                        .get()
                        .interrupt_depth
                        .load(Ordering::Relaxed),
                    2
                );
            }
            assert_eq!(
                INTERRUPT_STATE
                    .get()
                    .interrupt_depth
                    .load(Ordering::Relaxed),
                1
            );
        }
        assert_eq!(
            INTERRUPT_STATE
                .get()
                .interrupt_depth
                .load(Ordering::Relaxed),
            0
        );
    });

    roxy_test::kernel_test!("roxy-interrupt::eoi-policy", eoi_policy, {
        assert!(requires_eoi(LocalInterruptKind::Timer));
        assert!(requires_eoi(LocalInterruptKind::Error));
        assert!(!requires_eoi(LocalInterruptKind::Spurious));
    });
}
