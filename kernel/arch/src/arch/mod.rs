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
pub type SyscallHandler = fn(u64, u64) -> !;

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

    /// Enters ring 3 at the supplied instruction and stack pointers.
    ///
    /// # Safety
    ///
    /// Both addresses must be mapped as user-accessible in the active page table.
    unsafe fn enter_user(
        user_instruction_pointer: u64,
        user_stack_pointer: u64,
        kernel_stack_top: u64,
    ) -> !;

    fn configure_syscall(handler: SyscallHandler);

    fn set_kernel_stack_top(kernel_stack_top: u64);

    fn wait_for_interrupt();

    fn halt();

    fn halt_forever() -> !;
}

mod sealed {
    pub trait Sealed {}
}
