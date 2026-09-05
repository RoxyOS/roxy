use core::mem::{offset_of, size_of};
use core::time::Duration;

use roxy_memory::UserAddress;
use roxy_posix_timer::TimerClock;
use roxy_signal::Signal;
use strum::IntoEnumIterator;

use crate::{
    args::{SyscallArg, Timespec, user_memory},
    errno::Errno,
};

/// Flags for `timer_settime(2)`: `TIMER_ABSTIME` (1).
const TIMER_ABSTIME: u32 = 1;

/// Linux-compatible `sigevent.sigev_notify` values, fixed by the Roxy personality.
const SIGEV_SIGNAL: i32 = 0;
const SIGEV_NONE: i32 = 1;
const SIGEV_THREAD: i32 = 2;
const SIGEV_THREAD_ID: i32 = 4;

/// The Roxy `itimerspec` record: two [`Timespec`] values (`it_interval`, `it_value`), layout per
/// mlibc `bits/posix/posix_time.h`. Size 32, alignment 8.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Itimerspec {
    pub(super) interval: Timespec,
    pub(super) value: Timespec,
}

const _: () = assert!(size_of::<Itimerspec>() == 32);
const _: () = assert!(offset_of!(Itimerspec, interval) == 0);
const _: () = assert!(offset_of!(Itimerspec, value) == 16);

impl Itimerspec {
    /// Decodes a fully-validated `itimerspec` into its two durations.
    pub(super) const fn durations(self) -> (Duration, Duration) {
        (self.interval.duration(), self.value.duration())
    }
}

impl SyscallArg for Itimerspec {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut spec = Self {
            interval: Timespec::new(0, 0),
            value: Timespec::new(0, 0),
        };

        // SAFETY: Itimerspec has a checked C layout, contains only integers, and accepts every
        // bit pattern copied from userspace.
        unsafe { user_memory::read(address, &mut spec) }?;

        if !spec.interval.is_valid() || !spec.value.is_valid() {
            return Err(Errno::Invalid);
        }

        Ok(spec)
    }
}

/// The Roxy `sigevent` record, layout per mlibc `abi-bits/sigevent.h`.
///
/// Only the fields consumed by `timer_create` are named; the remaining pointer slots are zeroed
/// padding so the checked record accepts arbitrary userspace bytes. `sigev_value` is read as a
/// raw 64-bit word (the `union sigval` is 8 bytes) and passed through opaquely to `si_value`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct SigEvent {
    sigev_value: u64,
    sigev_notify: i32,
    sigev_signo: i32,
    sigev_notify_function: u64,
    sigev_notify_attributes: u64,
    sigev_notify_thread_id: i32,
    _pad: [u8; 4],
}

const _: () = assert!(size_of::<SigEvent>() == 40);
const _: () = assert!(offset_of!(SigEvent, sigev_notify) == 8);
const _: () = assert!(offset_of!(SigEvent, sigev_signo) == 12);
const _: () = assert!(offset_of!(SigEvent, sigev_value) == 0);

impl SyscallArg for SigEvent {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut event = Self {
            sigev_value: 0,
            sigev_notify: 0,
            sigev_signo: 0,
            sigev_notify_function: 0,
            sigev_notify_attributes: 0,
            sigev_notify_thread_id: 0,
            _pad: [0; 4],
        };

        // SAFETY: SigEvent has a checked C layout and every field is primitive, so it accepts
        // every userspace-supplied bit pattern.
        unsafe { user_memory::read(address, &mut event) }?;

        Ok(event)
    }
}

/// The notification configuration parsed out of a `timer_create` `sigevent`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TimerEvent {
    None,
    Signal { signal: Signal, value: u64 },
}

/// Decodes a `timer_create` `sigevent` (or the null default) into a notification configuration.
///
/// # Errors
///
/// Returns `Invalid` for an unchecked notification type or an invalid signal number. Unsupported
/// thread-directed notification modes report through the centralized diagnostic.
pub(super) fn decode_event(event: Option<SigEvent>) -> Result<TimerEvent, Errno> {
    // POSIX default: a null sevp is equivalent to `sigev_notify = SIGEV_SIGNAL` with `SIGALRM`.
    let (value, notify, signo) = match event {
        None => (0, SIGEV_SIGNAL, i32::from(Signal::Alarm.number())),
        Some(event) => (event.sigev_value, event.sigev_notify, event.sigev_signo),
    };

    match notify {
        SIGEV_NONE => Ok(TimerEvent::None),
        SIGEV_SIGNAL => {
            let signal = decode_signal(signo)?;
            Ok(TimerEvent::Signal { signal, value })
        }
        SIGEV_THREAD | SIGEV_THREAD_ID => Err(crate::unsupported::unsupported_argument(
            "timer_create.sigev_notify",
            notify,
            Errno::Invalid,
        )),
        _ => Err(Errno::Invalid),
    }
}

/// Maps a numeric signal to the matching [`Signal`].
fn decode_signal(number: i32) -> Result<Signal, Errno> {
    let number = u8::try_from(number).map_err(|_| Errno::Invalid)?;
    Signal::iter()
        .find(|signal| signal.number() == number)
        .ok_or(Errno::Invalid)
}

/// Interprets a `timer_settime` value+flags pair as an absolute monotonic deadline.
///
/// In absolute mode the value is measured against the timer's reference clock; in Roxy both
/// clocks share a monotonic base, so a `CLOCK_MONOTONIC` absolute value is used directly and a
/// `CLOCK_REALTIME` absolute value is shifted by the current realtime-vs-monotonic offset.
/// Relative mode adds the value to the current monotonic time. All arithmetic saturates.
///
/// A zero value in relative mode (or an absolute value already in the past) is handled by the
/// caller as a disarm request.
pub(super) fn deadline_from(clock: TimerClock, value: Duration, absolute: bool) -> Duration {
    let now_monotonic = roxy_time::monotonic_time();

    if absolute {
        match clock {
            TimerClock::Monotonic => value,
            TimerClock::Realtime => {
                let reference_offset = roxy_time::realtime_time().saturating_sub(now_monotonic);
                value.saturating_sub(reference_offset)
            }
        }
    } else {
        now_monotonic.saturating_add(value)
    }
}

/// The `timer_create`/`timer_settime` clock id, matching `clock_get`'s Linux-compatible ids.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ClockId {
    Realtime,
    Monotonic,
}

impl ClockId {
    pub(super) const fn timer_clock(self) -> TimerClock {
        match self {
            Self::Realtime => TimerClock::Realtime,
            Self::Monotonic => TimerClock::Monotonic,
        }
    }
}

impl SyscallArg for ClockId {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            0 => Ok(Self::Realtime),
            1 => Ok(Self::Monotonic),
            _ => Err(crate::unsupported::unsupported_argument(
                "timer_create.clockid",
                raw,
                Errno::Invalid,
            )),
        }
    }
}

/// The `timer_t` handle profile: a non-zero positive integer idle that the ABI stores in an
/// opaque `void *`. Decoded from the user's `timer_t` value and re-encoded into the `*timerid`
/// output slot of `timer_create`. Layout-neutral encoding is private to this personality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TimerHandle(u32);

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct TimerResult {
    id: u64,
}

const _: () = assert!(size_of::<TimerResult>() == 8);

impl TimerHandle {
    /// The userspace `timer_t` is an opaque pointer; the Roxy personality stores the small id in
    /// it directly (Linux-style), so decoding is the same value.
    pub(super) fn from_raw(raw: u64) -> Option<Self> {
        let id = u32::try_from(raw).ok()?;
        roxy_posix_timer::TimerId::new(id)?;
        Some(Self(id))
    }

    pub(super) fn timer_id(self) -> roxy_posix_timer::TimerId {
        // `TimerHandle` is only constructed from a value already validated as a nonzero
        // `TimerId`, so this cannot fail.
        roxy_posix_timer::TimerId::new(self.0).expect("timer handle is a nonzero timer id")
    }
}

impl SyscallArg for TimerHandle {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        TimerHandle::from_raw(raw).ok_or(error)
    }
}

/// Encodes a created timer id into the userspace `timer_t *timerid` output.
#[must_use]
pub(super) fn encode_timer_id(id: u32) -> TimerResult {
    TimerResult { id: u64::from(id) }
}

/// The `TIMER_ABSTIME` flag word for `timer_settime`.
pub(super) const fn is_absolute(flags: u32) -> Result<bool, Errno> {
    if flags & !TIMER_ABSTIME == 0 {
        Ok(flags & TIMER_ABSTIME != 0)
    } else {
        Err(Errno::Invalid)
    }
}
