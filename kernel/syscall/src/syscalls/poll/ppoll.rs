use crate::{
    SyscallResult,
    args::{Nullable, SignalMask, Timespec},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

use super::{PollEntriesAddress, poll};

syscall!(SyscallNumber::Ppoll, handle(entries: PollEntriesAddress => Fault, count: usize => Invalid, timeout: Nullable<Timespec> => Fault, signal_mask: Nullable<SignalMask> => Fault));

fn handle(
    entries: PollEntriesAddress,
    count: usize,
    timeout: Nullable<Timespec>,
    signal_mask: Nullable<SignalMask>,
) -> SyscallResult {
    let timeout = match timeout {
        Nullable::Null => None,
        Nullable::Value(timeout) => Some(timeout.duration()),
    };

    let old_mask = match signal_mask {
        Nullable::Null => None,
        Nullable::Value(signal_mask) => {
            Some(roxy_process::replace_masked_signals(signal_mask.to_vec()))
        }
    };

    let result = poll(entries, count, timeout);

    if matches!(result, Err(Errno::Interrupted)) {
        roxy_process::process_latest_signal();
    }

    if let Some(old_mask) = old_mask {
        roxy_process::replace_masked_signals(old_mask);
    }

    result
}
