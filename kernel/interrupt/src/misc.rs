use core::sync::atomic::Ordering;

use crate::{
    arch::{CurrentInterruptBackend, InterruptBackend},
    state::INTERRUPT_STATE,
};

pub(crate) fn record_apic_error() {
    let flags = CurrentInterruptBackend::error_flags();
    INTERRUPT_STATE
        .get()
        .last_apic_error
        .store(flags, Ordering::Relaxed);
    INTERRUPT_STATE
        .get()
        .apic_errors
        .fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn record_spurious() {
    INTERRUPT_STATE
        .get()
        .spurious_interrupts
        .fetch_add(1, Ordering::Relaxed);
}
