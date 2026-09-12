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

/// The information record written into a signal frame, laid out to match Roxy's userland
/// `siginfo_t` (`sysdeps/roxy/include/abi-bits/signal.h`).
///
/// The record is flat rather than a union overlay: a source sets the fields it has, and every
/// member's offset is then a constant instead of an overlay that `si_code` selects. Members keep
/// their POSIX names, so ported handlers compile unchanged. `si_overrun` and `si_value` keep the
/// offsets the Linux-shaped record gave them, so a handler or test that assumed those keeps
/// working; only the record's size changes.
// The `si_` prefix is the ABI's member naming, not accidental repetition: userspace reads these
// fields by these names, so they cannot be shortened.
#[allow(clippy::struct_field_names)]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Siginfo {
    si_signo: i32,
    si_code: i32,
    si_pid: i32,
    si_uid: u32,
    si_status: i32,
    si_overrun: i32,
    si_value: u64,
    si_addr: u64,
}

/// Size of the record as userspace sees it.
///
/// The frame layout and `sigtimedwait`'s output parameter both derive from this, so the record has
/// one size rather than one per user.
pub const SIGINFO_SIZE: usize = core::mem::size_of::<Siginfo>();

const _: () = assert!(SIGINFO_SIZE == 40);
const _: () = assert!(core::mem::offset_of!(Siginfo, si_code) == 4);
const _: () = assert!(core::mem::offset_of!(Siginfo, si_pid) == 8);
const _: () = assert!(core::mem::offset_of!(Siginfo, si_overrun) == 20);
const _: () = assert!(core::mem::offset_of!(Siginfo, si_value) == 24);
const _: () = assert!(core::mem::offset_of!(Siginfo, si_addr) == 32);

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

    match pending.source {
        // `si_value` publishes the timer's `sigval` payload. Roxy has no per-thread timer ids and
        // reports no overrun, so `si_overrun` stays zero.
        SignalSource::Timer => value.si_value = pending.value.unwrap_or(0),
        // `si_uid` stays zero: Roxy has no user model.
        SignalSource::Process | SignalSource::Tkill | SignalSource::Kernel => {
            value.si_pid = i32::try_from(pending.sender_pid).expect("pid fits in i32");
        }
    }

    // `si_addr` stays zero because no fault raises a signal yet, so there is no address to report.
    // TODO(signal-fault-delivery): the page-fault and exception paths must queue a signal carrying
    // the faulting address, which then fills `si_addr` and the fault-specific `si_code`s.

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
    RETURN_ADDRESS_SIZE, SIGINFO_OFFSET, SIGNAL_FRAME_SIZE, TRAMPOLINE_BASE, USER_CONTEXT_OFFSET,
    USER_CONTEXT_SIZE, build_bytes, restore_context, restore_old_mask, trampoline,
};

#[cfg(not(target_arch = "x86_64"))]
compile_error!("signal frames and the sigreturn trampoline are implemented for x86_64 only");
