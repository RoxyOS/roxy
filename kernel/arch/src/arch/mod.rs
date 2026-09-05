#[cfg(target_arch = "x86_64")]
mod x86_64;

use crate::CpuId;

#[cfg(target_arch = "x86_64")]
pub use self::x86_64::X86_64;

#[cfg(target_arch = "x86_64")]
pub type UserContext = self::x86_64::X86_64UserContext;

/// Byte length of the `syscall` instruction on the current architecture.
///
/// `SA_RESTART` rewinds the saved user instruction pointer by this amount so the CPU re-executes
/// the `syscall` instruction (and thus re-enters the kernel with the original arguments) after
/// the signal handler returns.
pub const SYSCALL_INSTRUCTION_SIZE: u64 = 2;

#[cfg(target_arch = "x86_64")]
pub type FloatState = self::x86_64::X86_64FloatState;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IrqLine(u8);

impl IrqLine {
    pub const ISA_COUNT: u8 = 16;

    #[must_use]
    pub const fn new(number: u8) -> Option<Self> {
        if number < Self::ISA_COUNT {
            Some(Self(number))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Interrupt {
    Local(LocalInterruptKind),
    Irq(IrqLine),
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::{Architecture, CurrentArchitectureBackend, Interrupt, IrqLine, LocalInterruptKind};

    roxy_test::kernel_test!("roxy-arch::irq-line-validation", irq_line_validation, {
        assert_eq!(IrqLine::new(0).unwrap().number(), 0);
        assert_eq!(IrqLine::new(15).unwrap().number(), 15);
        assert!(IrqLine::new(16).is_none());
    });

    roxy_test::kernel_test!("roxy-arch::irq-vector-mapping", irq_vector_mapping, {
        assert_eq!(
            CurrentArchitectureBackend::interrupt_vector(Interrupt::Irq(IrqLine::new(12).unwrap())),
            0x2c
        );
        assert_eq!(
            CurrentArchitectureBackend::interrupt_vector(Interrupt::Local(
                LocalInterruptKind::Timer
            )),
            0xf0
        );
    });
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
pub type InterruptDispatcher = fn(Interrupt);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
/// Normalized syscall arguments plus selected-backend user resume state.
pub struct RawSyscall {
    pub number: u64,
    pub arguments: [u64; 6],
    pub context: UserContext,
}

const _: () = {
    assert!(core::mem::offset_of!(RawSyscall, number) == 0);
    assert!(core::mem::offset_of!(RawSyscall, arguments) == 8);
};

/// Describes how to resume user code: the instruction to continue at, the stack to use, and the
/// first three argument registers.
///
/// Signal delivery uses it to resume into a user handler with the signal number as the first
/// argument; it mirrors the `resume_user` contract without resetting floating-point state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResumeInfo {
    pub instruction_pointer: u64,
    pub stack_pointer: u64,
    pub arguments: [u64; 3],
}

/// The exit state of one syscall.
#[derive(Clone, Copy, Debug)]
pub enum SyscallExit {
    /// Resumes the interrupted userspace context with `value` as the syscall result.
    Returned(u64),
    /// Resumes with `return_value` as the syscall result, but resumes into the user code
    /// described by `resume` first — currently the only producer is signal delivery into a user
    /// handler.
    Resume {
        return_value: u64,
        resume: ResumeInfo,
    },
    /// Replaces the entire saved context — including the syscall result — with the context
    /// restored from a signal frame. Used exclusively by `sigreturn`; never combined with
    /// delivery.
    RestoreContext(UserContext),
}

pub type SyscallHandler = fn(RawSyscall) -> SyscallExit;

pub trait Architecture: sealed::Sealed {
    fn initialize(exception_handler: ExceptionHandler);

    /// Registers the callback invoked by interrupt entry stubs.
    ///
    /// The backend converts the architecture-specific vector into an `Interrupt` before invoking
    /// `dispatcher`. Registration must happen exactly once while interrupts are disabled and before
    /// interrupts are enabled. The architecture layer owns entry mechanics only; the dispatcher
    /// owns interrupt policy and handler routing.
    fn register_interrupt_dispatcher(dispatcher: InterruptDispatcher);

    fn interrupt_vector(interrupt: Interrupt) -> u8;

    fn current_cpu_id() -> CpuId;

    /// Returns the current stack pointer value.
    fn current_stack_pointer() -> u64;

    /// Registers the current application processor's CPU identity and per-CPU floating-point
    /// state.
    ///
    /// This is the first action an AP takes after the bootloader hands over control. It assigns
    /// the next free slot in the bootloader-provided CPU map (the BSP already claims slot 0 during
    /// [`Architecture::initialize`]) and initializes this CPU's own x87/SSE state so the AP can run
    /// kernel code safely.
    ///
    /// `kernel_stack_top` is the top of the stack this CPU should use for ring-0 transitions
    /// (TSS RSP0 and syscall MSR).
    ///
    /// # Panics
    ///
    /// Panics when the current CPU is already registered or when more than `MAX_CPUS` CPUs
    /// register.
    fn initialize_application_processor(kernel_stack_top: u64);

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

    /// Returns directly from a syscall into a fresh user image.
    ///
    /// # Safety
    ///
    /// Both addresses must be mapped as user-accessible in the active page table.
    unsafe fn resume_user(instruction_pointer: u64, stack_pointer: u64) -> !;

    /// Selects the kernel stack used by every privileged entry from the active user thread.
    fn set_kernel_stack_top(kernel_stack_top: u64);

    fn user_thread_pointer() -> u64;

    /// Sets the architecture register used as the userspace thread pointer.
    ///
    /// # Panics
    ///
    /// Panics when `pointer` is not a canonical virtual address.
    fn set_user_thread_pointer(pointer: u64);

    fn wait_for_interrupt();

    fn halt();

    fn halt_forever() -> !;
}

mod sealed {
    pub trait Sealed {}
}
