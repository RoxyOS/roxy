#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
use self::x86_64::X86_64Interrupt;

#[cfg(target_arch = "x86_64")]
pub(crate) type CurrentInterruptBackend = X86_64Interrupt;

pub(crate) trait InterruptBackend: sealed::Sealed {
    fn initialize() -> u32;

    fn end_of_interrupt();

    fn error_flags() -> u8;
}

mod sealed {
    pub trait Sealed {}
}
