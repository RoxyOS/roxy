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
query readiness → register unready sources → prepare block(wait key)
→ source notification or deadline wake → re-query readiness → drop registrations
```

The caller performs query, registration, and block preparation with interrupts disabled. This
prevents an interrupt-driven source from changing state between observing it as unready and
recording its listener on the current BSP-only scheduler.

## Concurrency and limits

Wait queues are protected by their own locks. Notification can occur in interrupt context and does
not allocate; registration may allocate in thread context. Wakeups are advisory: a caller always
rechecks readiness after it resumes, so a notification may be spurious or match an event mask that
does not satisfy that caller. The crate currently has no fairness policy, edge-triggered mode, or
multi-CPU queue protocol.
