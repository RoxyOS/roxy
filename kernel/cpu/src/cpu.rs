use core::sync::atomic::{AtomicU64, Ordering};

use roxy_arch::{Architecture, CpuId, CurrentArchitectureBackend};

use crate::{
    CpuLocal,
    arch::{CpuBackend, CurrentCpuBackend},
};

static CPU_STATE: CpuLocal<CpuState> = CpuLocal::new();

struct CpuState {
    hardware_id: u32,
    interrupt_entries: AtomicU64,
}

impl CpuState {
    const fn new(hardware_id: u32) -> Self {
        Self {
            hardware_id,
            interrupt_entries: AtomicU64::new(0),
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

        let hardware_id = CurrentCpuBackend::initialize();
        CPU_STATE.initialize_current(CpuState::new(hardware_id));
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
        }
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
}
