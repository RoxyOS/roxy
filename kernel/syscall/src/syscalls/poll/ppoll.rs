use crate::{
    SyscallResult,
    args::{Nullable, SignalMask, Timespec},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
    unsupported::unsupported_argument,
};

use super::{PollEntriesAddress, poll};

syscall!(SyscallNumber::Ppoll, handle(entries: PollEntriesAddress => Fault, count: usize => Invalid, timeout: Nullable<Timespec> => Fault, signal_mask: Nullable<SignalMask> => Fault));

fn handle(
    entries: PollEntriesAddress,
    count: usize,
    timeout: Nullable<Timespec>,
    signal_mask: Nullable<SignalMask>,
) -> SyscallResult {
    let timeout = timeout.into_option();
    let signal_mask = signal_mask.into_option();

    if let Some(signal_mask) = signal_mask {
        let argument = if signal_mask.is_empty() {
            "empty"
        } else {
            "non-empty"
        };

        return Err(unsupported_argument(
            "ppoll.signal_mask",
            argument,
            Errno::NotSupported,
        ));
    }

    poll(entries, count, timeout.map(Timespec::duration))
}
