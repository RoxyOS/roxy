use roxy_arch::{Architecture, CpuId, CurrentArchitectureBackend};

use crate::CpuLocal;

pub(crate) static CPU_STATE: CpuLocal<CpuState> = CpuLocal::new();

pub(crate) struct CpuState {
    pub hardware_id: u32,
}

impl CpuState {
    const fn new(hardware_id: u32) -> Self {
        Self { hardware_id }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cpu {
    id: CpuId,
}

impl Cpu {
    /// Initializes this CPU's CPU-local state after hardware setup.
    ///
    /// # Panics
    ///
    /// Panics if this is not the current CPU or it was already initialized.
    pub fn initialize(self, hardware_id: u32) {
        self.assert_current();

        assert!(!CurrentArchitectureBackend::interrupts_enabled());
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
