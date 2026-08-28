use crate::{
    SyscallResult,
    args::{Nullable, SignalSet, Timespec},
    numbers::SyscallNumber,
    syscall,
};

use super::{PollEntriesAddress, poll};

syscall!(SyscallNumber::Ppoll, handle(entries: PollEntriesAddress => Fault, count: usize => Invalid, timeout: Nullable<Timespec> => Fault, signal_mask: Nullable<SignalSet> => Fault));

fn handle(
    entries: PollEntriesAddress,
    count: usize,
    timeout: Nullable<Timespec>,
    signal_mask: Nullable<SignalSet>,
) -> SyscallResult {
    let timeout = match timeout {
        Nullable::Null => None,
        Nullable::Value(timeout) => Some(timeout.duration()),
    };

    let old_mask = match signal_mask {
        Nullable::Null => None,
        Nullable::Value(signal_mask) => Some(roxy_process::replace_masked_signals(signal_mask)),
    };

    let result = poll(entries, count, timeout);

    if let Some(old_mask) = old_mask {
        roxy_process::replace_masked_signals(old_mask);
    }

    result
}
