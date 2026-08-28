use core::time::Duration;

use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

use super::{PollEntriesAddress, poll};

syscall!(SyscallNumber::Poll, handle(entries: PollEntriesAddress => Fault, count: usize => Invalid, timeout: PollTimeout => Invalid));

fn handle(entries: PollEntriesAddress, count: usize, timeout: PollTimeout) -> SyscallResult {
    poll(entries, count, timeout.0)
}

/// A millisecond timeout that treats `-1` as blocking indefinitely.
#[derive(Clone, Copy)]
struct PollTimeout(Option<Duration>);

impl SyscallArg for PollTimeout {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        match raw.cast_signed() {
            -1 => Ok(Self(None)),
            milliseconds if milliseconds >= 0 => Ok(Self(Some(Duration::from_millis(
                milliseconds.cast_unsigned(),
            )))),
            _ => Err(error),
        }
    }
}
