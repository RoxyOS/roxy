#![no_std]
#![feature(abi_x86_interrupt)]

mod arch;
mod cpuid;

pub use arch::{
    Architecture, CurrentArchitectureBackend, ExceptionContext, ExceptionHandler, ExceptionVector,
    FloatState, Interrupt, InterruptDispatcher, IrqLine, LocalInterruptKind, RawSyscall,
    ResumeInfo, SYSCALL_INSTRUCTION_SIZE, SyscallExit, SyscallHandler, UserContext, X86_64,
};
pub use cpuid::CpuId;
