#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
use self::x86_64::X86_64Cpu;

#[cfg(target_arch = "x86_64")]
pub(super) type CurrentCpuBackend = X86_64Cpu;

pub(super) trait CpuBackend: sealed::Sealed {
    fn initialize() -> u32;
}

mod sealed {
    pub trait Sealed {}
}
