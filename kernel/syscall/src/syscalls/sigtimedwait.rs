use core::time::Duration;

use roxy_signal::SignalSet;

use crate::{
    SyscallResult,
    args::{Nullable, Out, Timespec},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::SigtimedWait, handle(set: SignalSet => Fault, info: Nullable<Out<SiginfoAbi>> => Fault, timeout: Nullable<Timespec> => Fault, out_signal: Out<i32> => Fault));

/// The userspace `siginfo_t` (128 bytes) written to `info` for the consumed signal.
#[repr(C)]
#[derive(Clone, Copy)]
struct SiginfoAbi {
    bytes: [u8; 128],
}

const _: () = assert!(core::mem::size_of::<SiginfoAbi>() == 128);

/// Linux `sigtimedwait(2)`: suspends the calling thread until a signal in `set` is pending,
/// then consumes and returns it, writing its `siginfo_t` to `info` when requested.
///
/// Unlike normal signal delivery, a signal that the thread has blocked can be waited on here
/// (the `SIGEV_THREAD` helper-thread pattern). `timeout` bounds the wait; on expiry the call
/// returns `EAGAIN`.
fn handle(
    set: SignalSet,
    info: Nullable<Out<SiginfoAbi>>,
    timeout: Nullable<Timespec>,
    out_signal: Out<i32>,
) -> SyscallResult {
    let info = info.into_option();
    if let Some(info) = info {
        info.validate()?;
    }

    // No timeout blocks forever; any deadline is measured on the monotonic clock.
    let deadline = timeout.into_option().map_or(Duration::MAX, |spec| {
        roxy_time::monotonic_time().saturating_add(spec.duration())
    });

    let (signo, siginfo) = loop {
        if let Some(wait) = roxy_process::take_matching_pending_signal(set) {
            break (wait.number, wait.info);
        }

        if roxy_time::monotonic_time() >= deadline {
            // Linux returns EAGAIN when the timeout expires without a matching signal.
            return Err(Errno::Again);
        }

        // A pending unmasked signal other than the one being waited on will be delivered at the
        // return boundary, so abort with EINTR instead of swallowing it.
        if roxy_process::has_unmasked_pending_signal() {
            return Err(Errno::Interrupted);
        }

        roxy_timer_wait::block_current(deadline).perform();
    };

    // SAFETY: `i32` and `SiginfoAbi` are checked `repr(C)` records with every byte initialized.
    unsafe { out_signal.write(&signo) }?;
    if let Some(info) = info {
        let value = SiginfoAbi { bytes: siginfo };
        unsafe { info.write(&value) }?;
    }

    Ok(0)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use crate::numbers::SyscallNumber;

    kernel_test!(
        "roxy-syscall::sigtimedwait-registered",
        sigtimedwait_registered,
        {
            assert_eq!(SyscallNumber::try_from(81), Ok(SyscallNumber::SigtimedWait));
        }
    );
}
