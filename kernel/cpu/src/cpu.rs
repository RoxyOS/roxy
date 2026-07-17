use core::{
    sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering},
    time::Duration,
};

use roxy_arch::{Architecture, CpuId, CurrentArchitectureBackend};

use crate::{
    CpuLocal,
    arch::{CpuBackend, CurrentCpuBackend},
};

pub(crate) static CPU_STATE: CpuLocal<CpuState> = CpuLocal::new();

pub(crate) struct CpuState {
    pub hardware_id: u32,
    pub interrupt_depth: AtomicU32,
    pub interrupt_entries: AtomicU64,
    pub monotonic_nanos: AtomicU64,
    pub timer_ticks: AtomicU64,
    pub apic_errors: AtomicU64,
    pub last_apic_error: AtomicU8,
    pub spurious_interrupts: AtomicU64,
}

impl CpuState {
    const fn new(hardware_id: u32) -> Self {
        Self {
            hardware_id,
            interrupt_depth: AtomicU32::new(0),
            interrupt_entries: AtomicU64::new(0),
            monotonic_nanos: AtomicU64::new(0),
            timer_ticks: AtomicU64::new(0),
            apic_errors: AtomicU64::new(0),
            last_apic_error: AtomicU8::new(0),
            spurious_interrupts: AtomicU64::new(0),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cpu {
    id: CpuId,
}

impl Cpu {
    /// Initializes this CPU's architecture backend and CPU-local state.
    ///
    /// # Panics
    ///
    /// Panics if this is not the current CPU or it was already initialized.
    pub fn initialize(self) {
        self.assert_current();

        assert!(!CurrentArchitectureBackend::interrupts_enabled());
        let initialization = CurrentCpuBackend::initialize();
        CPU_STATE.initialize_current(CpuState::new(initialization.hardware_id));
        CurrentCpuBackend::start_timer();
    }

    #[must_use]
    pub const fn id(self) -> CpuId {
        self.id
    }

    #[must_use]
    pub fn hardware_id(self) -> u32 {
        self.assert_current();
        CPU_STATE.get().hardware_id
    }

    #[must_use]
    pub fn statistics(self) -> CpuStatistics {
        self.assert_current();
        CpuStatistics {
            interrupt_entries: CPU_STATE.get().interrupt_entries.load(Ordering::Relaxed),
            timer_ticks: CPU_STATE.get().timer_ticks.load(Ordering::Relaxed),
            apic_errors: CPU_STATE.get().apic_errors.load(Ordering::Relaxed),
            last_apic_error: CPU_STATE.get().last_apic_error.load(Ordering::Relaxed),
            spurious_interrupts: CPU_STATE.get().spurious_interrupts.load(Ordering::Relaxed),
        }
    }

    #[must_use]
    pub fn monotonic_time(self) -> Duration {
        self.assert_current();
        Duration::from_nanos(CPU_STATE.get().monotonic_nanos.load(Ordering::Relaxed))
    }

    fn assert_current(self) {
        assert_eq!(self.id, CurrentArchitectureBackend::current_cpu_id());
    }
}

#[must_use]
pub fn current_cpu() -> Cpu {
    Cpu {
        id: CurrentArchitectureBackend::current_cpu_id(),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CpuStatistics {
    pub interrupt_entries: u64,
    pub timer_ticks: u64,
    pub apic_errors: u64,
    pub last_apic_error: u8,
    pub spurious_interrupts: u64,
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::time::Duration;

    use roxy_arch::{Architecture, CurrentArchitectureBackend};

    use super::current_cpu;

    roxy_test::kernel_test!("roxy-cpu::periodic-timer-progresses", periodic_timer, {
        assert!(CurrentArchitectureBackend::interrupts_enabled());
        let cpu = current_cpu();
        let before_time = cpu.monotonic_time();
        let before = cpu.statistics();

        while cpu.statistics().timer_ticks < before.timer_ticks + 3 {
            CurrentArchitectureBackend::halt();
        }

        let after = cpu.statistics();
        assert!(cpu.monotonic_time() >= before_time + Duration::from_millis(12));
        assert!(after.interrupt_entries >= before.interrupt_entries + 3);
        assert_eq!(after.apic_errors, 0);
        assert_eq!(after.last_apic_error, 0);
    });
}
