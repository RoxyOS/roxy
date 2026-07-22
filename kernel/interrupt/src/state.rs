use core::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};

use roxy_arch::IrqLine;
use roxy_cpu::CpuLocal;

pub(crate) static INTERRUPT_STATE: CpuLocal<InterruptState> = CpuLocal::new();

pub(crate) struct InterruptState {
    pub interrupt_depth: AtomicU32,
    pub interrupt_entries: AtomicU64,
    pub apic_errors: AtomicU64,
    pub last_apic_error: AtomicU8,
    pub spurious_interrupts: AtomicU64,
    pub irq_entries: [AtomicU64; 16],
}

impl InterruptState {
    pub const fn new() -> Self {
        Self {
            interrupt_depth: AtomicU32::new(0),
            interrupt_entries: AtomicU64::new(0),
            apic_errors: AtomicU64::new(0),
            last_apic_error: AtomicU8::new(0),
            spurious_interrupts: AtomicU64::new(0),
            irq_entries: [const { AtomicU64::new(0) }; 16],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptStatistics {
    pub interrupt_entries: u64,
    pub apic_errors: u64,
    pub last_apic_error: u8,
    pub spurious_interrupts: u64,
    pub irq_entries: [u64; 16],
}

#[must_use]
/// Returns interrupt statistics for the current CPU.
///
/// # Panics
///
/// Panics when the interrupt subsystem is not initialized for the current CPU.
pub fn current_statistics() -> InterruptStatistics {
    let state = INTERRUPT_STATE.get();
    let irq_entries =
        core::array::from_fn(|index| state.irq_entries[index].load(Ordering::Relaxed));
    InterruptStatistics {
        interrupt_entries: state.interrupt_entries.load(Ordering::Relaxed),
        apic_errors: state.apic_errors.load(Ordering::Relaxed),
        last_apic_error: state.last_apic_error.load(Ordering::Relaxed),
        spurious_interrupts: state.spurious_interrupts.load(Ordering::Relaxed),
        irq_entries,
    }
}

pub(crate) fn record_irq(line: IrqLine) {
    let index = usize::from(line.number());
    let state = INTERRUPT_STATE.get();
    state.irq_entries[index].fetch_add(1, Ordering::Relaxed);
}
