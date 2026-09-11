use crate::{SyscallResult, args::Out, numbers::SyscallNumber, syscall};

use super::{ClockId, ClockResult};

syscall!(SyscallNumber::ClockGetres, handle(clock: ClockId => Invalid, output: Out<ClockResult> => Fault));

/// Reports the interval in which the requested clock advances, in the same record `clock_get`
/// uses.
///
/// A client uses this to learn how coarse a clock is, not to read it, so a caller that cannot
/// handle the reported interval falls back to another clock rather than failing.
fn handle(clock: ClockId, output: Out<ClockResult>) -> SyscallResult {
    let result = ClockResult::encode(clock.resolution());

    // SAFETY: `ClockResult`'s checked `repr(C)` layout contains two initialized integers without
    // implicit padding, so every byte of the object representation is defined.
    unsafe { output.write(&result) }?;

    Ok(0)
}
