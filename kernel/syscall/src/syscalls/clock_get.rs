use core::{mem::size_of, time::Duration};

use crate::{
    SyscallResult,
    args::{Out, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

syscall!(SyscallNumber::ClockGet, handle(clock: ClockId => Invalid, output: Out<ClockResult> => Fault));

const NANOS_PER_SECOND: u32 = 1_000_000_000;

#[repr(C)]
struct ClockResult {
    seconds: i64,
    nanoseconds: i64,
}

const _: () = assert!(size_of::<ClockResult>() == 16);

#[derive(Clone, Copy)]
enum ClockId {
    Realtime,
    Monotonic,
}

impl SyscallArg for ClockId {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            0 => Ok(Self::Realtime),
            1 => Ok(Self::Monotonic),
            _ => Err(crate::unsupported::unsupported_argument(
                "clock_get",
                raw,
                Errno::Invalid,
            )),
        }
    }
}

fn handle(clock: ClockId, output: Out<ClockResult>) -> SyscallResult {
    let time = match clock {
        ClockId::Realtime => roxy_time::realtime_time(),
        ClockId::Monotonic => roxy_time::monotonic_time(),
    };

    let result = encode(time);

    // SAFETY: ClockResult's checked repr(C) layout contains two initialized integers without
    // padding.
    unsafe { output.write(&result) }?;

    Ok(0)
}

fn encode(time: Duration) -> ClockResult {
    let overflowed = time.as_secs() > i64::MAX.cast_unsigned();
    let seconds = i64::try_from(time.as_secs()).unwrap_or(i64::MAX);
    let nanos = if overflowed {
        i64::from(NANOS_PER_SECOND - 1)
    } else {
        i64::from(time.subsec_nanos())
    };

    ClockResult {
        seconds,
        nanoseconds: nanos,
    }
}
