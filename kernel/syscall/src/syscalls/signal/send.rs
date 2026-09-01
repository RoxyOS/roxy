use roxy_process::{ProcessGroupId, ProcessId, SignalError, send_signal, send_signal_to_pgid};
use roxy_signal::Signal;

use crate::{
    SyscallResult, errno::Errno, numbers::SyscallNumber, syscall, unsupported::unsupported_argument,
};

syscall!(SyscallNumber::SendSignal, handle(pid: i64, signal: Signal => Invalid));

/// Linux `kill(2)` pid semantics: a positive pid targets one process, `0` targets the caller's
/// process group, `-1` targets every permitted process (unsupported here), and a value below
/// `-1` targets the process group with ID `-pid`.
fn handle(pid: i64, signal: Signal) -> SyscallResult {
    match pid {
        p if p > 0 => {
            let process_id = ProcessId::new(p.cast_unsigned()).ok_or(Errno::Invalid)?;
            send_signal(process_id, signal).map_err(|error| map_signal_error(error, signal))?;
        }
        0 => {
            let group = roxy_process::current_process_group_id();
            send_signal_to_pgid(group, signal);
        }
        -1 => {
            // TODO(process-wide-kill): Linux sends to every process the caller may signal
            // (excluding init); Roxy has no process-wide iteration helper yet.
            return Err(unsupported_argument(
                "kill.pid=-1",
                pid.cast_unsigned(),
                Errno::NotSupported,
            ));
        }
        p if p < -1 => {
            let group = ProcessGroupId::new((-p).cast_unsigned()).ok_or(Errno::Invalid)?;
            send_signal_to_pgid(group, signal);
        }
        _ => unreachable!("all i64 values are covered by the arms above"),
    }

    Ok(0)
}

fn map_signal_error(error: SignalError, signal: Signal) -> Errno {
    match error {
        SignalError::NoSuchProcess => Errno::NoSuchProcess,
        SignalError::UnsupportedAction => unsupported_argument(
            "send_signal.default_action",
            signal.number(),
            Errno::NotSupported,
        ),
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use crate::numbers::SyscallNumber;

    kernel_test!("roxy-syscall::kill-registered", kill_registered, {
        assert_eq!(SyscallNumber::try_from(35), Ok(SyscallNumber::SendSignal));
    });
}
