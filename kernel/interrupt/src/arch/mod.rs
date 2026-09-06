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

    /// Sends a fixed-delivery IPI with the given vector to the given target CPU using this
    /// controller. The caller owns vector selection; this is the raw hardware primitive.
    fn send_ipi(target: roxy_arch::CpuId, vector: u8);

    /// Broadcasts a stop-request NMI to every other CPU (all-except-self). This only delivers the
    /// NMI; whether and how a peer halts is owned by the caller/handler policy (the registered
    /// `ExceptionHandler` stops on `ExceptionVector::NonMaskable`, and the initiating core halts
    /// itself). NMI delivery reaches a peer even when it runs with interrupts disabled, so this
    /// is the primitive the whole-machine shutdown uses. Interrupts are disabled on this CPU
    /// before delivery and stay disabled.
    fn send_nmi();

    fn mask_irq(line: roxy_arch::IrqLine);

    fn unmask_irq(line: roxy_arch::IrqLine);
}

mod sealed {
    pub trait Sealed {}
}
