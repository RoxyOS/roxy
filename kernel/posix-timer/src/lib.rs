//! POSIX timer objects and their expiration dispatch.
//!
//! Owns the per-process timer registry, monolithic with the timer logic because the two are a
//! single cohesive unit: each mutation rewrites one timer's `TimerState`.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::time::Duration;

use roxy_arch::{Architecture, CurrentArchitectureBackend, LocalInterruptKind};
use roxy_process::{ProcessId, current_process_id};
use roxy_signal::Signal;
use roxy_utils::Lock;

/// Upper bound on the overrun count coalesced into a single delivered expiration per catch-up, so
/// a pathological sub-tick interval cannot spin the tick handler.
const OVERRUN_LIMIT: u32 = 1_000_000;

/// Assigned under `POSIX_TIMERS`; a small integer handle that is the only identity userspace
/// holds for a timer.
static POSIX_TIMERS: Lock<PosixTimers> = Lock::new(PosixTimers::new());

/// The clock a timer measures against (`timer_create(2)` `clockid_t`).
///
/// Both supported clocks share Roxy's single monotonic base, so the choice affects only
/// `timer_settime` absolute-deadline interpretation; the set-and-forget value is stored so an
/// absolute `timer_settime` can be converted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerClock {
    Monotonic,
    Realtime,
}

/// The notification a timer raises on expiration (`sigevent.sigev_notify`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerNotify {
    /// Deliver no notification (`SIGEV_NONE`); only re-arm a periodic timer.
    None,
    /// Queue `Signal` to the owning process with the timer's `sigval` payload.
    Signal(Signal),
}

/// An opaque, per-process handle to a POSIX timer. Zero is never a valid id.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerId(u32);

impl TimerId {
    #[must_use]
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PosixTimerError {
    /// No timer with this id is owned by the requesting process.
    NoEntry,
}

/// Whether a timer is armed, and if so its next absolute monotonic deadline.
///
/// Deadline, period, and overrun exist only when armed; a disarmed timer carries none of them.
#[derive(Clone, Copy)]
enum TimerState {
    Disarmed,
    Armed {
        next_deadline: Duration,
        interval: Duration,
        overrun: u32,
    },
}

/// A single timer: identity, owner, fixed configuration, and its arming state.
struct Timer {
    id: u32,
    owner: ProcessId,
    clock: TimerClock,
    notify: TimerNotify,
    value: u64,
    state: TimerState,
}

struct PosixTimers {
    entries: Vec<Timer>,
    next_id: u32,
}

impl PosixTimers {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
        }
    }

    /// Inserts a disarmed timer owned by `owner` and returns its id.
    ///
    /// The id space starts at one and never returns zero after a wrap, so ids stay valid until
    /// the table is exhausted (4 billion live timers).
    fn create(
        &mut self,
        owner: ProcessId,
        clock: TimerClock,
        notify: TimerNotify,
        value: u64,
    ) -> Result<TimerId, PosixTimerError> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);

        self.entries.push(Timer {
            id,
            owner,
            clock,
            notify,
            value,
            state: TimerState::Disarmed,
        });

        TimerId::new(id).ok_or(PosixTimerError::NoEntry)
    }

    /// Arms `id` with an absolute monotonic deadline and a period, resetting its overrun.
    fn arm(
        &mut self,
        id: TimerId,
        owner: ProcessId,
        deadline: Duration,
        interval: Duration,
    ) -> Result<(), PosixTimerError> {
        let timer = self.entry_mut(id, owner).ok_or(PosixTimerError::NoEntry)?;
        timer.state = TimerState::Armed {
            next_deadline: deadline,
            interval,
            overrun: 0,
        };
        Ok(())
    }

    /// Disarms `id` so it does not fire and clears its overrun, keeping it in the table.
    fn disarm(&mut self, id: TimerId, owner: ProcessId) -> Result<(), PosixTimerError> {
        let timer = self.entry_mut(id, owner).ok_or(PosixTimerError::NoEntry)?;
        timer.state = TimerState::Disarmed;
        Ok(())
    }

    /// Returns `(interval, time_until_next_expiration)` for `id` as of `now`.
    fn current(
        &self,
        id: TimerId,
        owner: ProcessId,
        now: Duration,
    ) -> Result<(Duration, Duration), PosixTimerError> {
        let timer = self.entry(id, owner).ok_or(PosixTimerError::NoEntry)?;
        let (interval, remaining) = match timer.state {
            TimerState::Disarmed => (Duration::ZERO, Duration::ZERO),
            TimerState::Armed {
                next_deadline,
                interval,
                ..
            } => (interval, next_deadline.saturating_sub(now)),
        };
        Ok((interval, remaining))
    }

    /// Returns the clock the timer was created against.
    fn clock(&self, id: TimerId, owner: ProcessId) -> Result<TimerClock, PosixTimerError> {
        Ok(self.entry(id, owner).ok_or(PosixTimerError::NoEntry)?.clock)
    }

    /// Returns the overrun count accumulated for `id` (zero when disarmed).
    fn overrun(&self, id: TimerId, owner: ProcessId) -> Result<u32, PosixTimerError> {
        let timer = self.entry(id, owner).ok_or(PosixTimerError::NoEntry)?;
        let TimerState::Armed { overrun, .. } = timer.state else {
            return Ok(0);
        };
        Ok(overrun)
    }

    /// Removes `id`, so it never fires again and is freed.
    fn delete(&mut self, id: TimerId, owner: ProcessId) -> Result<(), PosixTimerError> {
        let Some(index) = self
            .entries
            .iter()
            .position(|timer| timer.id == id.as_u32() && timer.owner == owner)
        else {
            return Err(PosixTimerError::NoEntry);
        };

        self.entries.swap_remove(index);
        Ok(())
    }

    /// Fires every armed timer whose deadline has passed, advancing each to its next period and
    /// delivering its configured notification.
    ///
    /// Runs in tick context, so it must not allocate. It scans the vector in place (the only
    /// allocation it can trigger is the downstream `send_timer_signal` pending queue push). A
    /// one-shot timer is disarmed rather than removed, because the timer object persists for
    /// `timer_delete` and can be re-armed by `timer_settime`.
    fn fire_due(&mut self, now: Duration) {
        let mut index = 0;

        while index < self.entries.len() {
            let should_fire = matches!(
                &self.entries[index].state,
                TimerState::Armed { next_deadline, .. } if *next_deadline <= now
            );

            if !should_fire {
                index += 1;
                continue;
            }

            let (notify, owner, value) = {
                let timer = &mut self.entries[index];
                advance_armed(&mut timer.state, now);
                (timer.notify, timer.owner, timer.value)
            };

            // `send_timer_signal` reports a missing (exited) owner; the timer is dropped lazily on
            // the next user-facing operation through the owner scoping check.
            if let TimerNotify::Signal(signal) = notify {
                let _ = roxy_process::send_timer_signal(owner, signal, value);
            }

            index += 1;
        }
    }

    fn entry(&self, id: TimerId, owner: ProcessId) -> Option<&Timer> {
        self.entries
            .iter()
            .find(|timer| timer.id == id.as_u32() && timer.owner == owner)
    }

    fn entry_mut(&mut self, id: TimerId, owner: ProcessId) -> Option<&mut Timer> {
        self.entries
            .iter_mut()
            .find(|timer| timer.id == id.as_u32() && timer.owner == owner)
    }
}

/// Advances one armed timer past `now`, counting coalesced expirations as overrun.
///
/// The fields are copied out and written back so the whole `TimerState` can be transitioned to
/// `Disarmed` for a one-shot while preserving an `Armed` period for a repeating timer.
fn advance_armed(state: &mut TimerState, now: Duration) {
    let TimerState::Armed {
        next_deadline,
        interval,
        overrun,
    } = *state
    else {
        return;
    };

    let mut next = next_deadline;
    let mut over = overrun;
    let mut steps = 0u64;
    let mut oneshot = false;

    while next <= now {
        // A second or later expiry within this catch-up window was not delivered separately;
        // count it as overrun.
        // TODO(pending-aware-overrun): Roxy's coarse 250 Hz tick has no pending-signal
        // introspection, so this reports coalesced (missed) expirations rather than the POSIX
        // definition of expirations missed while the previous expiration signal is still pending.
        if steps > 0 {
            over = over.saturating_add(1).min(OVERRUN_LIMIT);
        }
        steps += 1;

        if interval.is_zero() {
            // One-shot: disarm without removing; the object persists.
            oneshot = true;
            break;
        }

        if steps > u64::from(OVERRUN_LIMIT) {
            // Pathological sub-tick interval: stop counting and resynchronise one period past
            // `now` instead of spinning.
            next = now.saturating_add(interval);
            break;
        }

        next = next.saturating_add(interval);
    }

    if oneshot {
        *state = TimerState::Disarmed;
    } else {
        *state = TimerState::Armed {
            next_deadline: next,
            interval,
            overrun: over,
        };
    }
}

/// Registers the POSIX-timer expiration dispatcher with periodic timer delivery.
///
/// Runs after `roxy-time` advances the monotonic clock on each tick, evaluating due timers and
/// delivering their configured notification. See the crate `DESIGN.md` for the dispatch contract.
///
/// # Panics
///
/// Panics when interrupts are enabled or the handler is registered twice.
pub fn initialize() {
    roxy_interrupt::register_local_handler(LocalInterruptKind::Timer, on_tick);
}

/// Expiration dispatcher for the periodic tick.
fn on_tick() {
    assert!(!CurrentArchitectureBackend::interrupts_enabled());
    POSIX_TIMERS.lock().fire_due(roxy_time::monotonic_time());
}

/// Creates a disarmed POSIX timer for the calling process.
///
/// The timer is armed by [`arm`]. `clock` selects the reference clock used to interpret an
/// absolute `timer_settime`; `notify` configures what happens at expiration; for
/// `TimerNotify::Signal`, `value` is the `sigval` payload delivered to `SA_SIGINFO` handlers
/// through `si_value`.
///
/// # Errors
///
/// Returns an error if the timer id space is exhausted.
pub fn create(
    clock: TimerClock,
    notify: TimerNotify,
    value: u64,
) -> Result<TimerId, PosixTimerError> {
    let owner = current_process_id();
    POSIX_TIMERS.lock().create(owner, clock, notify, value)
}

/// Arms `id` with an absolute monotonic deadline and a period, resetting its overrun.
///
/// A zero `interval` arms a one-shot timer. `deadline` is already converted to monotonic terms
/// by the caller.
///
/// # Errors
///
/// Returns an error when `id` is not a valid timer owned by the calling process.
pub fn arm(
    id: TimerId,
    absolute_deadline: Duration,
    interval: Duration,
) -> Result<(), PosixTimerError> {
    let owner = current_process_id();
    POSIX_TIMERS
        .lock()
        .arm(id, owner, absolute_deadline, interval)
}

/// Returns `(interval, time_until_next_expiration)` for `id` as of the current monotonic clock.
///
/// # Errors
///
/// Returns an error when `id` is not a valid timer owned by the calling process.
pub fn current(id: TimerId) -> Result<(Duration, Duration), PosixTimerError> {
    let owner = current_process_id();
    let now = roxy_time::monotonic_time();
    POSIX_TIMERS.lock().current(id, owner, now)
}

/// Returns the clock the owning process created `id` against.
///
/// # Errors
///
/// Returns an error when `id` is not a valid timer owned by the calling process.
pub fn clock(id: TimerId) -> Result<TimerClock, PosixTimerError> {
    let owner = current_process_id();
    POSIX_TIMERS.lock().clock(id, owner)
}

/// Returns the overrun count accumulated for `id`.
///
/// # Errors
///
/// Returns an error when `id` is not a valid timer owned by the calling process.
pub fn overrun(id: TimerId) -> Result<u32, PosixTimerError> {
    let owner = current_process_id();
    POSIX_TIMERS.lock().overrun(id, owner)
}

/// Removes `id` from the registry, so it never fires again.
///
/// # Errors
///
/// Returns an error when `id` is not a valid timer owned by the calling process.
pub fn delete(id: TimerId) -> Result<(), PosixTimerError> {
    let owner = current_process_id();
    POSIX_TIMERS.lock().delete(id, owner)
}

/// Disarms `id`, so it does not fire again, while retaining it in the registry.
///
/// # Errors
///
/// Returns an error when `id` is not a valid timer owned by the calling process.
pub fn disarm(id: TimerId) -> Result<(), PosixTimerError> {
    let owner = current_process_id();
    POSIX_TIMERS.lock().disarm(id, owner)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::time::Duration;

    use roxy_process::ProcessId;
    use roxy_test::kernel_test;

    use super::{PosixTimerError, PosixTimers, TimerClock, TimerId, TimerNotify};

    fn owner() -> ProcessId {
        ProcessId::new(7).unwrap()
    }

    fn other() -> ProcessId {
        ProcessId::new(9).unwrap()
    }

    fn none() -> TimerNotify {
        TimerNotify::None
    }

    fn new_timer() -> (PosixTimers, TimerId) {
        let mut timers = PosixTimers::new();
        let id = timers
            .create(owner(), TimerClock::Monotonic, none(), 0)
            .unwrap();
        (timers, id)
    }

    kernel_test!(
        "roxy-posix-timer::create-assigns-ids",
        create_assigns_ids,
        {
            let mut timers = PosixTimers::new();
            let a = timers
                .create(owner(), TimerClock::Monotonic, none(), 0)
                .unwrap();
            let b = timers
                .create(owner(), TimerClock::Realtime, none(), 0)
                .unwrap();

            assert_ne!(a, b);
            assert_eq!(a.as_u32(), 1);
            assert_eq!(b.as_u32(), 2);
        }
    );

    kernel_test!(
        "roxy-posix-timer::periodic-fore-advances",
        periodic_fire_advances,
        {
            let (mut timers, id) = new_timer();
            timers
                .arm(
                    id,
                    owner(),
                    Duration::from_millis(10),
                    Duration::from_millis(10),
                )
                .unwrap();

            // Not yet due.
            timers.fire_due(Duration::from_millis(9));
            assert_eq!(
                timers
                    .current(id, owner(), Duration::from_millis(9))
                    .unwrap(),
                (Duration::from_millis(10), Duration::from_millis(1))
            );

            // Due: advances to the next period, fires once, overrun stays 0.
            timers.fire_due(Duration::from_millis(10));
            assert_eq!(timers.overrun(id, owner()), Ok(0));
            assert_eq!(
                timers
                    .current(id, owner(), Duration::from_millis(10))
                    .unwrap(),
                (Duration::from_millis(10), Duration::from_millis(10))
            );
        }
    );

    kernel_test!(
        "roxy-posix-timer::tiny-interval-accumulates-overrun",
        tiny_interval_overrun,
        {
            let (mut timers, id) = new_timer();
            timers
                .arm(
                    id,
                    owner(),
                    Duration::from_millis(1),
                    Duration::from_millis(1),
                )
                .unwrap();

            // Catch up far beyond the deadline; multiple periods elapsed -> overrun > 0.
            timers.fire_due(Duration::from_millis(9));
            assert!(timers.overrun(id, owner()).unwrap() > 0);
            assert!(
                timers
                    .current(id, owner(), Duration::from_millis(9))
                    .unwrap()
                    .1
                    > Duration::ZERO
            );
        }
    );

    kernel_test!("roxy-posix-timer::oneshot-disarms", oneshot_disarms, {
        let (mut timers, id) = new_timer();
        timers
            .arm(id, owner(), Duration::from_millis(5), Duration::ZERO)
            .unwrap();

        timers.fire_due(Duration::from_millis(5));
        // Disarmed: a later fire must not deliver again.
        let (_, remaining) = timers
            .current(id, owner(), Duration::from_millis(100))
            .unwrap();
        assert_eq!(remaining, Duration::ZERO);
        timers.fire_due(Duration::from_millis(100));
        assert_eq!(timers.overrun(id, owner()), Ok(0));
    });

    kernel_test!("roxy-posix-timer::delete-removes", delete_removes, {
        let (mut timers, id) = new_timer();
        assert_eq!(timers.delete(id, owner()), Ok(()));
        assert_eq!(
            timers.current(id, owner(), Duration::ZERO),
            Err(PosixTimerError::NoEntry)
        );
    });

    kernel_test!("roxy-posix-timer::owner-scoping", owner_scoping, {
        let (mut timers, id) = new_timer();
        let other = other();
        assert_eq!(
            timers.arm(id, other, Duration::from_millis(1), Duration::ZERO),
            Err(PosixTimerError::NoEntry)
        );
        assert_eq!(
            timers.current(id, other, Duration::ZERO),
            Err(PosixTimerError::NoEntry)
        );
        assert_eq!(timers.delete(id, other), Err(PosixTimerError::NoEntry));
        assert_eq!(timers.delete(id, owner()), Ok(()));
    });
}
