#![no_std]
#![feature(abi_x86_interrupt)]

mod arch;
mod cpuid;

pub use arch::{
    Architecture, CurrentArchitectureBackend, ExceptionContext, ExceptionHandler, ExceptionVector,
    LocalInterruptHandler, LocalInterruptKind, SyscallHandler, X86_64,
};
pub use cpuid::CpuId;
