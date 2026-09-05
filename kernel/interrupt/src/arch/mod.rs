#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
use self::x86_64::X86_64Interrupt;

#[cfg(target_arch = "x86_64")]
pub(crate) type CurrentInterruptBackend = X86_64Interrupt;

pub(crate) trait InterruptBackend: sealed::Sealed {
    fn initialize(platform: crate::InterruptPlatformInfo) -> u32;

    /// Initialises the local APIC of an application processor without the BSP-only IOAPIC setup.
    fn initialize_ap() -> u32;

    fn end_of_interrupt();

    fn error_flags() -> u8;

    fn mask_irq(line: roxy_arch::IrqLine);

    fn unmask_irq(line: roxy_arch::IrqLine);
}

mod sealed {
    pub trait Sealed {}
}
