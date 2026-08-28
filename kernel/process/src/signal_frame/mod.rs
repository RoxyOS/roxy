//! Signal frame construction and the kernel-injected `sigreturn` trampoline.
//!
//! The kernel maps [`trampoline`] — a tiny read-execute code stub that issues `sigreturn` — on a
//! dedicated page in every process image and points each signal handler's return address at it,
//! so user programs never need their own signal restorer. The `sigreturn` kernel handler restores
//! the interrupted context recorded in the frame below the handler's stack pointer.
//!
//! The frame layout and trampoline bytes are architecture contracts owned by the per-architecture
//! submodule; [`SIGRETURN_SYSCALL_NUMBER`] and the `siginfo_t` are the architecture-independent
//! pieces. Supporting a new architecture means adding a sibling submodule under a
//! `cfg(target_arch)` arm.

use crate::signal::{PendingSignal, SignalSource};

/// Syscall number of `sigreturn` in the Roxy ABI.
///
/// Must match `SyscallNumber::Sigreturn` in `roxy-syscall`; a kernel test pins both sides.
pub const SIGRETURN_SYSCALL_NUMBER: u64 = 54;

/// The Linux-compatible `siginfo_t` (musl-derived `abis/linux/signal.h` layout), architecture
/// neutral for the ABIs Roxy targets.
///
/// Only the fields the kernel can produce are named; the remaining `si_*` slots stay zeroed.
/// `_pad` models the alignment that pushes the pid/uid union to offset 16.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Siginfo {
    si_signo: i32,
    si_errno: i32,
    si_code: i32,
    _pad: i32,
    si_pid: i32,
    si_uid: u32,
    _rest: [u8; 104],
}

const _: () = assert!(core::mem::size_of::<Siginfo>() == 128);
const _: () = assert!(core::mem::offset_of!(Siginfo, si_code) == 8);
const _: () = assert!(core::mem::offset_of!(Siginfo, si_pid) == 16);

/// Linux `si_code` values, used only at this ABI-serialization boundary.
const SI_USER: i32 = 0;
const SI_TKILL: i32 = -6;
const SI_KERNEL: i32 = 128;

/// Builds the `siginfo_t` for a pending signal.
///
/// Maps the ABI-neutral [`SignalSource`] to the Linux `si_code` integer only here, at the ABI
/// boundary, so the process layer never depends on an ABI's numeric conventions.
#[must_use]
pub(super) fn build_siginfo(pending: PendingSignal) -> Siginfo {
    // SAFETY: `Siginfo` is a POD of `i32`/`u32`/`u8` fields, so an all-zero bit pattern is a valid
    // `siginfo_t`; `si_errno`, `si_uid`, and the `_rest` tail stay zeroed.
    let mut value = unsafe { core::mem::zeroed::<Siginfo>() };
    value.si_signo = i32::from(pending.signal.number());
    value.si_code = abi_si_code(pending.source);
    value.si_pid = i32::try_from(pending.sender_pid).expect("pid fits in i32");
    value
}

fn abi_si_code(source: SignalSource) -> i32 {
    match source {
        SignalSource::Process => SI_USER,
        SignalSource::Tkill => SI_TKILL,
        SignalSource::Kernel => SI_KERNEL,
    }
}

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub(super) use x86_64::{
    SIGINFO_OFFSET, SIGNAL_FRAME_SIZE, TRAMPOLINE_BASE, UCONTEXT_OFFSET, USER_CONTEXT_OFFSET,
    USER_CONTEXT_SIZE, build_bytes, restore_context, restore_old_mask, trampoline,
};

#[cfg(not(target_arch = "x86_64"))]
compile_error!("signal frames and the sigreturn trampoline are implemented for x86_64 only");
