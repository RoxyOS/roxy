# Posix Timer Design

## Purpose and scope

`roxy-posix-timer` owns the POSIX timer object model and its expiration dispatch: per-process
timer ids, their clock and notification configuration, absolute monotonic deadlines, periodic
re-arming, one-shot disarm, and overrun accounting. It is the kernel-side target backing the
`timer_create`/`timer_settime`/`timer_gettime`/`timer_getoverrun`/`timer_delete` ABI surface,
which the syscall subsystem adapts.

It does **not** own the monotonic or realtime clocks (`roxy-time` does), hardware timer
configuration, thread-blocking deadline wakeups (`roxy-timer-wait` does), or the syscall
`itimerspec`/`sigevent` record layouts (the syscall subsystem does). It also does not deliver
signal frames; it queues a process signal through `roxy-process`.

## Ownership and flow

Each timer is created for the calling process (its owner) and stores:

- an opaque `TimerId` (a nonzero `u32`) that is the only handle userspace holds;
- the owner `ProcessId`, used to scope the timer id to its creating process;
- the selected `TimerClock` (both clocks share Roxy's monotonic base, so it only affects
  `timer_settime` absolute-deadline interpretation and is stored so that conversion can run
  against a later `timer_settime` call);
- the notification configuration: `SIGEV_NONE` or a `Signal` to raise with a `sigval` payload;
- an arming `state` represented by the `TimerState` enum: `Disarmed`, or `Armed` carrying a
  monotonic absolute `next_deadline`, a `Duration` period (`interval`), and a cumulative `overrun`
  count. Deadline, period, and overrun exist only in the `Armed` variant, so a disarmed timer
  cannot accidentally retain one.

The timer logic and its registry live together in `src/lib.rs`: each operation maps one timer's
`TimerState` (arm/disarm rewrite it wholesale; `fire_due` advances it through a helper that
returns the new `TimerState`), so there is no separate submodule boundary worth naming.

Timers are registered in a global table guarded by `roxy_utils::Lock`. `timer_create` inserts a
disarmed timer; `timer_settime` (via `arm`) sets its deadline and period and arms it; `delete`
removes it; and `timer_gettime`/`timer_getoverrun` read it back. The expiration dispatcher runs
as a periodic-tick handler:

```text
timer tick (after roxy-time advances the monotonic clock)
→ fire_due(now)
   → for each active timer with next_deadline <= now:
       advance next_deadline; count coalesced expirations as overrun
       deliver SIGEV_SIGNAL via roxy-process::send_timer_signal
       (SIGEV_NONE only re-arms)
```

This path is intentionally decoupled from `roxy-timer-wait`: a POSIX timer must expire even while
the owning process is running (not blocked), which `roxy-timer-wait` cannot represent. `fire_due`
is the one tick registration; it is added by `initialize()` after `roxy-time`'s clock handler so
it observes an already-advanced clock.

## Concurrency and limits

- User-facing operations (`create`/`arm`/`current`/`overrun`/`delete`) run in syscall context.
- `fire_due` runs in interrupt context with interrupts disabled; it must not allocate (it scans
  the vector in place, and the only allocation it can trigger is the `send_timer_signal` pending
  queue push, an accepted precedent shared with the terminal ISIG path).
- The timer table lock never nests inside the process-table lock in the opposite order; the
  interrupt-path `send_timer_signal` acquires the process-table lock while already holding the
  timer lock, and no code takes the timer lock while holding the process lock.
- A timer whose owning process has exited is removed lazily when `send_timer_signal` reports a
  missing process.
- Deadlines and periods use saturated `Duration` arithmetic and a bounded overrun advance loop so
  a pathological tiny period cannot spin the tick handler.

## Overrun semantics (approximation)

`timer_getoverrun` reports the number of timer expirations coalesced into the single delivered
notification when the dispatcher catches the timer up: after the triggering expiry, each further
complete period that elapsed before the dispatcher returned to `now` counts as overrun. Roxy has
a coarse 250 Hz tick and no pending-signal introspection, so this is an approximation of the
POSIX definition (expirations missed while a prior expiration signal is still pending and not
delivered). It is not burst-infinite: `SIGEV_SIGNAL` timers deliver at most one signal per
processed batch, and overrun counting is bounded. Follow-up work toward true pending-aware
overrun is tracked by a `TODO(<missing-capability>)` in `src/lib.rs` and in `ISSUES.md`.