use spin::Mutex;
use x2apic::lapic::{LocalApic, LocalApicBuilder};

use crate::CpuLocal;

use super::{CpuArchitecture, sealed};

const TIMER_VECTOR: usize = 0xf0;
const ERROR_VECTOR: usize = 0xfe;
const SPURIOUS_VECTOR: usize = 0xff;

static LOCAL_APIC: CpuLocal<Mutex<X2Apic>> = CpuLocal::new();

pub(crate) struct X86_64Cpu;

struct X2Apic {
    _local_apic: LocalApic,
}

// SAFETY: The builder receives no xAPIC base, so every successfully built value uses MSR-only x2APIC.
unsafe impl Send for X2Apic {}

impl sealed::Sealed for X86_64Cpu {}

impl CpuArchitecture for X86_64Cpu {
    fn initialize() -> u32 {
        let mut builder = LocalApicBuilder::new();
        builder
            .timer_vector(TIMER_VECTOR)
            .error_vector(ERROR_VECTOR)
            .spurious_vector(SPURIOUS_VECTOR);
        let mut local_apic = builder.build().unwrap();

        // SAFETY: The builder selected x2APIC, and this is the BSP's unique local controller.
        unsafe {
            local_apic.enable();
            local_apic.disable_timer();
            assert!(local_apic.is_bsp());
        }

        // SAFETY: The local controller is enabled and uniquely borrowed.
        let hardware_id = unsafe { local_apic.id() };
        LOCAL_APIC.initialize_current(Mutex::new(X2Apic {
            _local_apic: local_apic,
        }));
        hardware_id
    }
}
