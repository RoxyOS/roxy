# Timer Wait Design

## Purpose and scope

`roxy-timer-wait` owns deadline registrations for blocked threads. It converts periodic monotonic
clock progress into token-checked scheduler wakeups. It does not own the monotonic clock, hardware
timer configuration, context switching, or resource-specific wait conditions.

## Ownership and flow

A caller with interrupts disabled invokes `block_current` to register the current thread and its absolute
monotonic deadline, then passes the generated wait key to the scheduler while preparing the block.
Timer interrupts
run after `roxy-time` advances the clock, remove expired registrations, and ask the scheduler to
wake each thread only when that same key remains active.

```text
register deadline + scheduler block(key)
→ timer tick advances monotonic time
→ timer-wait removes expired registration
→ scheduler wake_if_waiting(thread, key)
```

The wait key identifies one registration rather than a thread. A stale expiry therefore cannot
wake a later wait by the same thread.

## Concurrency and limits

The deadline vector is protected by the timer-wait lock. Registration may allocate; timer interrupt
processing only removes existing entries and must not allocate. The current implementation scans
all registrations on each tick. A caller with another wake source explicitly calls
`cancel_wakeup_deadline` after it resumes to remove an unexpired deadline. A missing entry during
cancellation means timer processing already consumed it; scheduler wait keys still reject stale
expiry notifications from an earlier block.

`register_wakeup_deadline` only creates that wakeup source. It never marks a thread blocked or
switches contexts; callers that combine a deadline with another source must register every source,
then prepare and perform one scheduler block with their shared wait key, and explicitly cancel the
deadline after an early wakeup.
