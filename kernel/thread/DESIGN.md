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
→ update kernel stack top → save outgoing context → switch stacks → release outgoing CPU
```

Each scheduler entry owns a stable `Box` and a per-entry `reserved: Box<AtomicBool>`. Entry storage and
the `reserved` flag are never reallocated or moved for the lifetime of a `ThreadIndex`, so a pointer or
index published under the scheduler lock stays valid while another CPU enqueues, and reaping leaves
vacant slots rather than shifting indexes. `SavedContext` likewise lives in a stable, separately
owned allocation rather than being moved with the scheduler `Vec`.

## Per-CPU scheduler state

The scheduler separates shared run-queue state from per-CPU state. The shared `Scheduler` under
one `Lock` holds the global runnable queue (`entries`) and deferred reap bookkeeping. Which thread
is running on which CPU, and each CPU's control-context save area, is per-CPU: a `LocalScheduler`
slot (`current` plus `control_context`) per possible CPU, indexed by the current architecture CPU
id and stored outside the shared lock in `LOCAL`.

The switch methods operate on both halves: they read or update the current CPU's `LocalScheduler`
slot and the shared queue while the guard is held, then perform the returned switch only after
releasing it. A CPU touches only its own slot, and only from its own scheduler context (control
loop, timer preemption, block, exit, wake), so no cross-CPU aliasing of a slot ever occurs.

The current thread cannot be removed while its kernel stack is active. Exit marks it `Exiting`,
records a pending reap, switches away, and removes the entry on a later scheduler pass running on a
different stack.

Blocking records a caller-owned wait key in the scheduler state. A wake source must present the
same key to make the thread runnable, so a stale notification from an earlier wait cannot affect a
later wait by the same thread. Resource-specific queues and deadline registration belong to their
owning subsystems rather than the scheduler.

A keyed block may pass a caller-owned wake latch (`prepare_block_current_with_key_and_latch`). The
caller's notifier sets the latch before asking the scheduler to wake, so a wake that reaches a still-
running thread (which `wake_if_waiting` drops) is recorded instead of lost; when the latch is set the
thread does not block at all - no context switch is prepared, it keeps running, and the caller
re-checks its readiness. The thread is never marked `Runnable` before its switch away: marking a
still-running thread runnable and switching later would expose it to concurrent dispatch on SMP
(the "Runnable while still running" window), so an owed wake simply skips the block. Both the latch
store and the consuming swap run through the scheduler lock, so the owed wake cannot be lost on SMP.
The scheduler only holds the latch across the block preparation call, never after it.

## CPU ownership handoff

A thread is reserved for its CPU the moment the scheduler marks it `Running` (`reserved = true`, done
under the scheduler lock). Dispatch, `next_runnable`, and reap all skip or defer a thread while its
`reserved` flag is set, so an early wake (`Blocked → Runnable`) or a preemption (`Running → Runnable`)
cannot let another CPU dispatch a thread that is still executing on its own stack.

The assembly `switch_context` handoff clears `reserved` with an ordered store only after it has saved
the outgoing context into `[rdi]` and switched `rsp` to the incoming stack. Heap-based atomics and
context allocations keep the flag and the saved state at stable absolute addresses, so the release
store remains reachable after the stack swap. Reaping therefore never frees a kernel stack while
that stack still holds the outgoing thread.

The `running` thread is never reaped while `reserved` is set; reaping removes the entry only after
the handoff cleared the flag, and it takes the boxed slot leaving a vacant `Option`, so another
CPU's recorded `current` index always stabilizes to the same thread.

## User dispatch hook

The thread crate cannot depend on the process crate because process already depends on thread.
Process therefore registers one `UserDispatchHook` before the scheduler starts. Immediately before
switching to a user thread, scheduler invokes the hook with the target `ThreadId`.

The hook is responsible for resolving that target thread's owning process and activating the
address space currently stored by the process. It runs after the scheduler lock is released and
with interrupts disabled. It must complete synchronously and cannot leave the wrong page table
active. A kernel/control-context target bypasses the hook and activates the kernel page table.

## Interrupt registration

`roxy-thread::initialize` registers the scheduler's timer-preemption handler with
`roxy-interrupt`. Registration occurs during boot with interrupts disabled and must complete before
the time subsystem unmasks periodic timer delivery. The handler runs after interrupt accounting and
EOI, with interrupts disabled, and may perform a context switch; the interrupt subsystem therefore
does not retain scheduler policy or call this handler directly.

The timer-wait subsystem registers its handler between the time and scheduler handlers during
composition-root initialization. Each tick therefore advances monotonic time, wakes matching
deadline waiters, then applies ordinary scheduler preemption policy.

## Invariants and limits

- Context switching never occurs while the scheduler lock is held.
- A thread is never dispatched or reaped while its `reserved` flag is set.
- A thread is reaped only after execution has moved off its kernel stack.
- Blocking code must prepare the block while protecting its wait queue, release that queue's lock,
  then perform the switch.
- The scheduler validates wait keys but does not own resource-specific wait queues or deadlines.
- The dispatch hook is registered once during boot before any user thread runs.

The bootstrap processor runs the whole boot: it is always cleared to dispatch, and it runs the
initial process itself. Application processors each run their own scheduler control loop with a
per-CPU timer but are held behind an `APS_READY` gate until `kernel-main` has spawned the initial
process, so they never steal the boot thread. After readiness they dispatch runnable threads from
the shared queue.

Idle application processors are tracked in a shared `IDLE` array and woken by a reschedule IPI
whenever a thread becomes runnable (`enqueue`/`wake`), so a free AP picks up work immediately
rather than waiting for its next timer tick. A thread is marked `Running` on dispatch so two CPUs
cannot grab the same thread. The run queue still round-robins by a flat index and there is no load
balancing, CPU affinity, or process-level multi-threading policy.

The scheduler receives opaque wait keys from higher-level wait sources. It does not own timer
deadlines or relative-duration policy. Timed waits currently have no signal interruption or
remaining-duration reporting.

## Rejected alternative

Storing an address-space handle in each scheduler entry duplicates process ownership and leaves a
stale handle after `execve`. Resolving the current process-owned handle at dispatch time preserves
the ownership boundary and makes image replacement visible automatically.
