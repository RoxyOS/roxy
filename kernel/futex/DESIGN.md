# Futex Design

## Purpose and scope

`roxy-futex` implements address-space-local waits and wakes on aligned 32-bit user words. It owns
the wait table and integrates with thread blocking; it does not implement mutexes, condition
variables, or userspace scheduling policy.

## Key identity and ownership

A futex key is `(AddrSpaceId, UserAddress)`. The address-space component prevents unrelated
processes from waking one another even when virtual addresses match. The global table owns queues
of waiting `ThreadId` values; the scheduler owns the thread objects and blocked/runnable states.

## Wait and wake flow

`wait` validates the key, reads the user word, and compares it while holding the futex table lock.
If the value matches, it enqueues the current thread, prepares a scheduler block, releases the
table lock, and performs the switch. Holding the table lock across the value check and enqueue
prevents a wake from being lost between those operations.

`wake` removes up to the requested count and asks the scheduler to make each thread runnable. A
thread-exit callback removes all remaining wait entries for that thread.

## Invariants and failures

Addresses must be 4-byte aligned. Unmapped reads return `Fault`, value changes return `Mismatch`,
and invalid alignment returns `Invalid`. Cleanup must run before the thread is finally reaped so
the table never retains an exited thread id.

The current implementation uses one global lock and supports one kernel scheduler model; it does
not provide priority inheritance, timeout queues, or cross-address-space futexes.
