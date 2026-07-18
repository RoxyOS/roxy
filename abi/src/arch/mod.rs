#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub(crate) type CurrentArchitectureBackend = x86_64::X86_64ArchitectureBackend;

pub(crate) trait ArchitectureBackend: sealed::Sealed {
    /// Invokes a one-argument syscall that must not return.
    ///
    /// # Safety
    ///
    /// `number` must identify a syscall whose ABI accepts `argument` in the architecture's first
    /// argument register and never returns.
    unsafe fn syscall1_noreturn(number: u64, argument: u64) -> !;
}

mod sealed {
    pub(crate) trait Sealed {}
}
