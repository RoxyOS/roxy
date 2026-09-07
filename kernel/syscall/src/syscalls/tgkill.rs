use roxy_process::{SignalError, send_thread_signal};
use roxy_signal::Signal;
use roxy_thread::ThreadId;

use crate::{
    SyscallResult, errno::Errno, numbers::SyscallNumber, syscall, unsupported::unsupported_argument,
};

syscall!(SyscallNumber::Tgkill, handle(tgid: i64, tid: i64, signal: Signal => Invalid));

/// Linux `tgkill(2)`: sends `signal` to the thread `tid` of the process `tgid`.
///
/// Roxy allows a process to target only its own threads (equivalent to Linux's rule that the
/// caller must belong to `tgid`), so `tgid` must be the calling process and `tid` one of its
/// threads.
#[allow(clippy::similar_names)]
fn handle(tgid: i64, tid: i64, signal: Signal) -> SyscallResult {
    let current = roxy_process::current_process_id();
    if tgid != current.as_u64().cast_signed() {
        return Err(Errno::NoSuchProcess);
    }

    let Ok(tid_value) = u64::try_from(tid) else {
        return Err(Errno::Invalid);
    };
    let Some(thread_id) = ThreadId::from_u64(tid_value) else {
        return Err(Errno::Invalid);
    };

    send_thread_signal(thread_id, signal).map_err(|error| map_error(error, signal))?;

    Ok(0)
}

fn map_error(error: SignalError, signal: Signal) -> Errno {
    match error {
        SignalError::NoSuchProcess => Errno::NoSuchProcess,
        SignalError::UnsupportedAction => unsupported_argument(
            "tgkill.default_action",
            signal.number(),
            Errno::NotSupported,
        ),
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use crate::numbers::SyscallNumber;

    kernel_test!("roxy-syscall::tgkill-registered", tgkill_registered, {
        assert_eq!(SyscallNumber::try_from(82), Ok(SyscallNumber::Tgkill));
    });
}
