use crate::{SyscallResult, args::Out, numbers::SyscallNumber, syscall};

use super::{ClockId, ClockResult};

syscall!(SyscallNumber::ClockGet, handle(clock: ClockId => Invalid, output: Out<ClockResult> => Fault));

/// Reports one `roxy_clock_result` for the requested clock.
fn handle(clock: ClockId, output: Out<ClockResult>) -> SyscallResult {
    let result = ClockResult::encode(clock.now());

    // SAFETY: `ClockResult`'s checked `repr(C)` layout contains two initialized integers without
    // implicit padding, so every byte of the object representation is defined.
    unsafe { output.write(&result) }?;

    Ok(0)
}
