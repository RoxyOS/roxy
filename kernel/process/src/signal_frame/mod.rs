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

/// The `siginfo_t` written into a signal frame, laid out to match Roxy's userland
/// `siginfo_t` (`sysdeps/roxy/include/abi-bits/signal.h`). Only the fields the kernel can produce
/// are named; the fault/poll/sys members Roxy never raises stay zero.
///
/// `_pad` pushes `sifields` to offset 16, matching the alignment of the userland `__si_fields`
/// union (its 8-byte-aligned members force it past the 12-byte header). `sifields` is a union
/// because the region is a runtime-typed overlay: at offset 16..24 it is either `_kill`
/// (`si_pid`/`si_uid`) or `_timer` (`si_tid`/`si_overrun`), and `si_value` follows at 24.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Siginfo {
    si_signo: i32,
    si_errno: i32,
    si_code: i32,
    _pad: i32,
    sifields: Sifields,
}

/// The `siginfo_t` union at offset 16 (the userland `__si_fields` overlay). Only the `kill` and
/// `timer` variants Roxy produces are named; `_pad` sizes the union to the 112-byte tail so the
/// whole `siginfo_t` is 128 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) union Sifields {
    kill: SifieldKill,
    timer: SifieldTimer,
    _pad: [u8; 112],
}

/// The `_kill` variant: `si_pid`/`si_uid` at 16.
#[repr(C)]
#[derive(Clone, Copy)]
struct SifieldKill {
    pid: i32,
    uid: u32,
}

/// The `_timer` variant: `si_tid`/`si_overrun` at 16, `si_value` at 24.
#[repr(C)]
#[derive(Clone, Copy)]
struct SifieldTimer {
    tid: i32,
    overrun: i32,
    value: u64,
}

const _: () = assert!(core::mem::size_of::<Siginfo>() == 128);
const _: () = assert!(core::mem::offset_of!(Siginfo, si_code) == 8);
const _: () = assert!(core::mem::offset_of!(Siginfo, sifields) == 16);
const _: () = assert!(core::mem::offset_of!(SifieldKill, pid) == 0);
const _: () = assert!(core::mem::offset_of!(SifieldTimer, value) == 8);
// `si_value` sits at sifields (16) + timer.value's offset (8) = 24.
const _: () = assert!(
    core::mem::offset_of!(Siginfo, sifields) + core::mem::offset_of!(SifieldTimer, value) == 24
);

/// Linux `si_code` values, used only at this ABI-serialization boundary.
const SI_USER: i32 = 0;
const SI_TIMER: i32 = -2;
const SI_TKILL: i32 = -6;
const SI_KERNEL: i32 = 128;

/// Builds the `siginfo_t` for a pending signal.
///
/// Maps the ABI-neutral [`SignalSource`] to the Linux `si_code` integer only here, at the ABI
/// boundary, so the process layer never depends on an ABI's numeric conventions.
#[must_use]
pub(super) fn build_siginfo(pending: PendingSignal) -> Siginfo {
    // SAFETY: `Siginfo` is POD over a union whose every variant accepts an all-zero bit pattern,
    // so an all-zero representation is a valid `siginfo_t`; `si_errno` and the unraised fields
    // stay zeroed.
    let mut value = unsafe { core::mem::zeroed::<Siginfo>() };
    value.si_signo = i32::from(pending.signal.number());
    value.si_code = abi_si_code(pending.source);

    if pending.source == SignalSource::Timer {
        // Write the `timer` variant (the handler reads the same variant). `si_tid`/`si_overrun`
        // stay zero (Roxy has no per-thread timer ids and reports no overrun); `si_value`
        // publishes the timer's `sigval` payload.
        value.sifields.timer.tid = 0;
        value.sifields.timer.overrun = 0;
        value.sifields.timer.value = pending.value.unwrap_or(0);
    } else {
        // Write the `kill` variant; `si_uid` stays zero.
        value.sifields.kill.pid = i32::try_from(pending.sender_pid).expect("pid fits in i32");
        value.sifields.kill.uid = 0;
    }

    value
}

fn abi_si_code(source: SignalSource) -> i32 {
    match source {
        SignalSource::Process => SI_USER,
        SignalSource::Timer => SI_TIMER,
        SignalSource::Tkill => SI_TKILL,
        SignalSource::Kernel => SI_KERNEL,
    }
}

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub(super) use x86_64::{
    RETURN_ADDRESS_SIZE, SIGINFO_OFFSET, SIGNAL_FRAME_SIZE, TRAMPOLINE_BASE, UCONTEXT_OFFSET,
    USER_CONTEXT_OFFSET, USER_CONTEXT_SIZE, build_bytes, restore_context, restore_old_mask,
    trampoline,
};

#[cfg(not(target_arch = "x86_64"))]
compile_error!("signal frames and the sigreturn trampoline are implemented for x86_64 only");
