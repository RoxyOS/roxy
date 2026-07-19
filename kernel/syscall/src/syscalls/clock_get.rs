use core::{mem::size_of, slice, time::Duration};

use roxy_memory::UserAddress;

use crate::{Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::ClockGet, handle);

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

impl ClockId {
    fn parse(clock: u64) -> Result<Self, Errno> {
        match clock {
            0 => Ok(Self::Realtime),
            1 => Ok(Self::Monotonic),
            _ => Err(crate::unsupported::unsupported_argument(
                "clock_get",
                clock,
                Errno::Invalid,
            )),
        }
    }
}

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let clock = ClockId::parse(arguments[0])?;
    let output = UserAddress::new(arguments[1]).ok_or(Errno::Fault)?;

    let time = match clock {
        ClockId::Realtime => roxy_time::realtime_time(),
        ClockId::Monotonic => roxy_time::monotonic_time(),
    };

    let result = encode(time);

    // SAFETY: ClockResult is repr(C), contains only initialized integer fields, and the slice
    // lives only for the duration of the write.
    let bytes = unsafe {
        slice::from_raw_parts((&raw const result).cast::<u8>(), size_of::<ClockResult>())
    };

    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;

    addrspace
        .write_bytes(output, bytes)
        .map_err(|_| Errno::Fault)?;

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
