#[cfg(target_arch = "x86_64")]
mod x86_64;

use crate::CpuId;

#[cfg(target_arch = "x86_64")]
pub use self::x86_64::X86_64;

#[cfg(target_arch = "x86_64")]
pub type CurrentArchitectureBackend = X86_64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExceptionVector {
    DivideError,
    InvalidOpcode,
    DoubleFault,
    GeneralProtectionFault,
    PageFault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalInterruptKind {
    Timer,
    Error,
    Spurious,
}

#[derive(Clone, Copy, Debug)]
pub struct ExceptionContext {
    pub vector: ExceptionVector,
    pub error_code: Option<u64>,
    pub instruction_pointer: u64,
    pub stack_pointer: u64,
    pub code_segment: u64,
    pub stack_segment: u64,
    pub cpu_flags: u64,
    pub fault_address: Option<u64>,
    pub cpu_id: CpuId,
}

pub type ExceptionHandler = fn(&ExceptionContext) -> !;
pub type LocalInterruptHandler = fn(LocalInterruptKind);

pub trait Architecture: sealed::Sealed {
    fn initialize(
        exception_handler: ExceptionHandler,
        local_interrupt_handler: LocalInterruptHandler,
    );

    fn local_interrupt_vector(kind: LocalInterruptKind) -> u8;

    fn current_cpu_id() -> CpuId;

    fn interrupts_enabled() -> bool;

    fn without_interrupts<T>(function: impl FnOnce() -> T) -> T;

    fn enable_interrupts();

    fn halt();

    fn halt_forever() -> !;
}

mod sealed {
    pub trait Sealed {}
}
