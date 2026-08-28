//! Signal frame construction and the kernel-injected `sigreturn` trampoline.
//!
//! The kernel maps [`trampoline`] — a tiny read-execute code stub that issues `sigreturn` — on a
//! dedicated page in every process image and points each signal handler's return address at it,
//! so user programs never need their own signal restorer. The `sigreturn` kernel handler restores
//! the interrupted context recorded in the frame below the handler's stack pointer.
//!
//! The frame layout and trampoline bytes are architecture contracts owned by the per-architecture
//! submodule; [`SIGRETURN_SYSCALL_NUMBER`] is the one architecture-independent piece. Supporting a
//! new architecture means adding a sibling submodule under a `cfg(target_arch)` arm.

/// Syscall number of `sigreturn` in the Roxy ABI.
///
/// Must match `SyscallNumber::Sigreturn` in `roxy-syscall`; a kernel test pins both sides.
pub const SIGRETURN_SYSCALL_NUMBER: u64 = 54;

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub(super) use x86_64::{
    SIGNAL_FRAME_SIZE, TRAMPOLINE_BASE, USER_CONTEXT_OFFSET, USER_CONTEXT_SIZE, build_bytes,
    restore_context, restore_old_mask, trampoline,
};

#[cfg(not(target_arch = "x86_64"))]
compile_error!("signal frames and the sigreturn trampoline are implemented for x86_64 only");
