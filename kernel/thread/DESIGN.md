# Thread and Scheduler Design

## Purpose and scope

`roxy-thread` owns thread identity, kernel stacks, saved contexts, runnable state, context switching,
blocking, waking, preemption, exit, and deferred reaping. It does not own process identity,
descriptor tables, or user address spaces.

## Ownership model

The scheduler owns each `Thread`, including its kernel stack and saved context, until deferred
reaping removes the entry. A scheduler entry records only whether the thread is kernel or user; it
does not retain an `AddrSpaceHandle`. Process address spaces remain owned by the process table so
`execve` can replace them without updating scheduler state.

On x86_64 each saved context also owns one aligned FXSAVE image. Every context switch eagerly saves
the outgoing x87/MMX/SSE state and restores the incoming state. A new process starts from the
architectural default, while fork captures the current parent's state for the child. This keeps
asynchronous preemption from leaking SIMD state between threads.

## Context-switch flow

Every dispatch, preemption, block, or exit produces a `PendingContextSwitch` while the scheduler
lock is held. The switch is performed only after the lock guard is released:

```text
select target → release scheduler lock → prepare target address space
→ update kernel stack top → switch saved context
```

The current thread cannot be removed while its kernel stack is active. Exit marks it `Exiting`,
records a pending reap, switches away, and removes the entry on a later scheduler pass running on a
different stack.

## User dispatch hook

The thread crate cannot depend on the process crate because process already depends on thread.
Process therefore registers one `UserDispatchHook` before the scheduler starts. Immediately before
switching to a user thread, scheduler invokes the hook with the target `ThreadId`.

The hook is responsible for resolving that target thread's owning process and activating the
address space currently stored by the process. It runs after the scheduler lock is released and
with interrupts disabled. It must complete synchronously and cannot leave the wrong page table
active. A kernel/control-context target bypasses the hook and activates the kernel page table.

## Invariants and limits

- Context switching never occurs while the scheduler lock is held.
- A thread is reaped only after execution has moved off its kernel stack.
- Blocking code must prepare the block while protecting its wait queue, release that queue's lock,
  then perform the switch.
- The dispatch hook is registered once during boot before any user thread runs.

The scheduler is currently global and BSP-oriented. It has no priorities, CPU affinity, SMP run
queues, or process-level multi-threading policy.

## Rejected alternative

Storing an address-space handle in each scheduler entry duplicates process ownership and leaves a
stale handle after `execve`. Resolving the current process-owned handle at dispatch time preserves
the ownership boundary and makes image replacement visible automatically.
