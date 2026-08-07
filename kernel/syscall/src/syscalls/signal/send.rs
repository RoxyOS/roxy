use roxy_process::{ProcessId, SignalError, send_signal};
use roxy_signal::Signal;

use crate::{
    SyscallResult, errno::Errno, numbers::SyscallNumber, syscall, unsupported::unsupported_argument,
};

syscall!(SyscallNumber::SendSignal, handle(process_id: ProcessId => Invalid, signal: Signal => Invalid));

fn handle(process_id: ProcessId, signal: Signal) -> SyscallResult {
    send_signal(process_id, signal).map_err(|error| map_signal_error(error, signal))?;

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
