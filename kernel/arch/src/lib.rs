#![no_std]

mod arch;
mod cpuid;

pub use arch::{Architecture, CurrentArchitecture, X86_64};
pub use cpuid::CpuId;
