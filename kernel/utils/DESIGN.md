# Kernel Utilities Design

## Purpose and scope

`roxy-utils` contains small cross-cutting kernel mechanisms that do not belong to one service:
preemption-aware locking, preemption guards, and centralized unsupported-operation reporting. It
must remain narrow and must not become a dumping ground for unrelated helpers.

## Lock and preemption contract

`Lock<T>` disables preemption before acquiring its spin mutex. `LockGuard` drops the mutex guard
before restoring preemption, preventing the current CPU from being rescheduled while it owns a
non-sleeping kernel lock. Guards are not transferable between CPUs.

Preemption disablement is nestable and represented by an RAII guard. The current implementation is
BSP-only; unbalanced drops, cross-CPU drops, and depth overflow are kernel faults.

Callers must not perform a context switch while holding `LockGuard`. A wait path must update its
protected state, prepare the scheduler transition, release the lock, and only then switch.

## Unsupported-operation reporting

The serial reporter is installed once during core initialization. Any missing or unsupported
userspace operation must call the centralized reporter before returning its errno. The diagnostic
includes operation, argument, process id, thread id, and errno and cannot be compiled into a silent
fallback.

## Limits

Utilities should be promoted into an owning subsystem when they acquire policy, lifecycle, or
multiple domain-specific callers. General convenience functions are not sufficient reason to add
to this crate.
