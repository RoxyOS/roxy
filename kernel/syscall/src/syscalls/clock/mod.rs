//! Shared ABI of the clock syscalls.
//!
//! Both clock reads report the same `struct roxy_clock_result` record, and both accept the same
//! `clockid_t` values, so the record layout and the clock set live here rather than in either
//! handler. The record and the identifiers are mirrored by
//! `sysdeps/roxy/include/roxy/syscall.h` in the Roxy mlibc fork.

use core::{mem::size_of, time::Duration};

use crate::{args::SyscallArg, errno::Errno, unsupported::unsupported_argument};

pub(super) mod get;
pub(super) mod getres;

pub(super) const GET_SYSCALL: crate::Syscall = get::SYSCALL;
pub(super) const GETRES_SYSCALL: crate::Syscall = getres::SYSCALL;

const NANOS_PER_SECOND: u32 = 1_000_000_000;

/// Roxy `struct roxy_clock_result`: one clock reading split into whole seconds and nanoseconds.
#[repr(C)]
pub(super) struct ClockResult {
    seconds: i64,
    nanoseconds: i64,
}

const _: () = assert!(size_of::<ClockResult>() == 16);

impl ClockResult {
    /// Splits `time` into whole seconds and remaining nanoseconds, saturating at `i64::MAX`.
    pub(super) fn encode(time: Duration) -> Self {
        let overflowed = time.as_secs() > i64::MAX.cast_unsigned();
        let seconds = i64::try_from(time.as_secs()).unwrap_or(i64::MAX);
        let nanoseconds = if overflowed {
            i64::from(NANOS_PER_SECOND - 1)
        } else {
            i64::from(time.subsec_nanos())
        };

        Self {
            seconds,
            nanoseconds,
        }
    }
}

/// The `clockid_t` values the kernel provides.
#[derive(Clone, Copy)]
pub(super) enum ClockId {
    Realtime,
    Monotonic,
}

impl ClockId {
    /// Decodes a raw `clockid_t`, or `None` for an identifier the kernel does not provide.
    pub(super) const fn from_raw(raw: u64) -> Option<Self> {
        match raw {
            0 => Some(Self::Realtime),
            1 => Some(Self::Monotonic),
            _ => None,
        }
    }

    /// Returns this clock's current reading.
    pub(super) fn now(self) -> Duration {
        match self {
            Self::Realtime => roxy_time::realtime_time(),
            Self::Monotonic => roxy_time::monotonic_time(),
        }
    }

    /// Returns the interval in which this clock advances.
    ///
    /// The realtime base is a fixed offset from the monotonic clock, so both advance in the same
    /// periodic-timer ticks and the reading is one tick.
    #[allow(clippy::unused_self)] // the interval belongs to the clock it describes
    pub(super) fn resolution(self) -> Duration {
        roxy_time::resolution()
    }
}

impl SyscallArg for ClockId {
    /// Decodes a raw `clockid_t`.
    ///
    /// The identifier names its own argument in the diagnostic rather than the calling syscall,
    /// because `clock_get` and `clock_getres` accept the same set.
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        Self::from_raw(raw).ok_or_else(|| unsupported_argument("clockid", raw, Errno::Invalid))
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::time::Duration;

    use roxy_test::kernel_test;

    use super::{ClockId, ClockResult};

    kernel_test!(
        "roxy-syscall::clock-result-encoding",
        splits_seconds_and_nanos,
        {
            let result = ClockResult::encode(Duration::new(3, 250));

            assert_eq!(result.seconds, 3);
            assert_eq!(result.nanoseconds, 250);
        }
    );

    kernel_test!(
        "roxy-syscall::clock-result-overflow",
        saturates_at_maximum,
        {
            let result = ClockResult::encode(Duration::new(u64::MAX, 1));

            assert_eq!(result.seconds, i64::MAX);
            assert_eq!(result.nanoseconds, 999_999_999);
        }
    );

    kernel_test!("roxy-syscall::clock-ids", decodes_provided_clocks, {
        assert!(matches!(ClockId::from_raw(0), Some(ClockId::Realtime)));
        assert!(matches!(ClockId::from_raw(1), Some(ClockId::Monotonic)));
        assert!(ClockId::from_raw(6).is_none());
    });
}
