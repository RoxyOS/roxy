use crate::CpuId;

use super::{Architecture, sealed};

pub struct X86_64;

impl sealed::Sealed for X86_64 {}

impl Architecture for X86_64 {
    fn current_cpu_id() -> CpuId {
        CpuId::BSP
    }

    fn interrupts_enabled() -> bool {
        ::x86_64::instructions::interrupts::are_enabled()
    }

    fn without_interrupts<T>(function: impl FnOnce() -> T) -> T {
        ::x86_64::instructions::interrupts::without_interrupts(function)
    }

    fn halt() {
        ::x86_64::instructions::hlt();
    }

    fn halt_forever() -> ! {
        ::x86_64::instructions::interrupts::disable();

        loop {
            Self::halt();
        }
    }
}
