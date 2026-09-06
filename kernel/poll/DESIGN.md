# Poll Wait Design

## Purpose and scope

`roxy-poll` owns registrations for one blocked readiness wait. It provides a token-checked listener,
per-source wait queues, and RAII cancellation of registrations. It does not decode userspace
`pollfd` layouts, define descriptor readiness, own a timer queue, or decide which event masks are
reported.

## Ownership and flow

A caller creates one `PollListener` for a blocking attempt and passes an `Arc` of it to each source
that is currently not ready. Sources retain the listener in their own `PollListeners`; the returned
`PollRegistration` removes precisely that queue entry on drop. A source whose state may have
changed calls `notify`, which asks the scheduler to wake the listener only when its current block has
the same wait key.

```text
register listeners → re-check readiness → prepare block with wake latch
→ source notification or deadline wake → re-query readiness → drop registrations
```

The caller performs registration, re-check, and block preparation with interrupts disabled, and
blocks under the scheduler lock against a caller-provided wake latch. This order is required for
SMP: registering every listener before the readiness decision means a source that becomes ready
mid-registration is still observed (either by the re-check or through a notification on the
now-registered listener), and the latch turns a notification delivered while the owner thread was
still `Running` (which `wake_if_waiting` drops) into a non-blocking wake instead of a lost one: an
owed wake skips the block entirely (the thread keeps running and re-checks) rather than marking a
still-running thread runnable.

## Concurrency and limits

Wait queues are protected by their own locks. Notification can occur in interrupt context and does
not allocate; registration may allocate in thread context. Wakeups are advisory: a caller always
rechecks readiness after it resumes, so a notification may be spurious or match an event mask that
does not satisfy that caller. The crate currently has no fairness policy, edge-triggered mode, or
multi-CPU queue protocol beyond the scheduler's wake latch.

The wake latch is owned by the `PollListener` and released with it, so no cross-CPU pending-wake
state leaks out of the registration lifetime; the scheduler only consumes the latch it is handed at
block time.
