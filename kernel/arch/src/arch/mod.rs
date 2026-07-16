mod x86_64;

use crate::CpuId;

pub use self::x86_64::X86_64;

pub type CurrentArchitecture = X86_64;

pub trait Architecture: sealed::Sealed {
    fn current_cpu_id() -> CpuId;

    fn interrupts_enabled() -> bool;

    fn without_interrupts<T>(function: impl FnOnce() -> T) -> T;

    fn halt();

    fn halt_forever() -> !;
}

mod sealed {
    pub trait Sealed {}
}
