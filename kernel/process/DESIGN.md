# Process Design

## Purpose and scope

`roxy-process` owns process identity, process state, the process table, process-owned address
spaces, descriptor tables, process creation, fork, `execve`, and process-facing virtual-memory
operations. It does not own scheduler context switching, ELF parsing policy, or VFS storage.

## Ownership and dependency boundaries

Each process owns exactly one optional `AddrSpaceHandle`, one set of user threads, and one `FdTable`;
the first thread created is the main thread, which anchors `main_thread_id`. All threads of a process
share the same address space, descriptor table, working directory, umask, and signal dispositions,
while each thread carries its own kernel stack, saved context, and signal mask. The process table
maps thread ids to process ids so the thread scheduler can request address-space activation without
depending on this crate.

Each process owns one normalized absolute working directory. Directly spawned processes start at
the VFS root, fork children clone the parent's directory, and `execve` preserves it with the other
process metadata. Process initialization registers a VFS working-directory provider that clones the
current directory under the process-table lock. The VFS invokes it only for relative global
operations and receives an owned snapshot, so no process-table lock spans path normalization or
filesystem access. The syscall subsystem uses the same owned-snapshot API for `getcwd`, keeping
userspace memory access outside the process-table lock.

Each process also records an optional parent process ID. Directly spawned processes have no parent;
fork children record the caller's process ID. Only that recorded parent may wait for and remove the
child's exited process-table entry. An exited parent remains visible until its own parent waits for
it; removing the entry clears every child's matching parent ID, making those children orphans.

The scheduler owns threads and saved contexts, but never owns a process address-space handle. The
ELF and VM crates provide construction primitives; process decides when a constructed image becomes
published.

Resources that a client takes on behalf of a process are released through the process-exit
notification (`register_process_exit_handler`), a single reverse-dependency slot that `roxy-fbdev`
uses to free the framebuffer's visible frame when its owner exits. It complements the
session-leader-exit handler by covering release that is tied to the process rather than to a
descriptor: an owner that exits without closing the descriptor still frees the resource.

## Threads

A process is created with one main thread, and `create_thread` adds a runnable user thread that
shares the process's address space, descriptor table, and signal state. Each thread owns a kernel
stack and saved context (via `roxy-thread`); the caller supplies the already-mapped user stack, so
the kernel does not allocate thread stacks. The thread-create/exit/gettid syscalls back the mlibc
pthread implementation, so real user threads exist; the `thread_owners` map is the single registry
recording which threads belong to each process.

Thread reaping keys process finalization on the last remaining thread: `finish_thread_reap` removes
the reaped thread from `thread_owners`, and only when no thread of the process remains does it
transition the process to `Exited`, release its address space, and wake waiters. A non-last thread
reaping therefore leaves the process running. Thread-targeted delivery (`tgkill`, `sigtimedwait`,
`SIGEV_THREAD_ID`) queues a signal in the specific thread's per-thread state, which is how a
`SIGEV_THREAD` helper thread waits on the timer's signal.

Process-directed signals are delivered by waking the target process's main thread when it is still
scheduled, and fall back to any other live thread of the process otherwise, so a signal is not lost
once the main thread has reaped. Thread-directed delivery (`tgkill`, `SIGEV_THREAD_ID`) queues the
signal in the specific thread's per-thread pending queue and wakes it. Process-directed selection
does not yet prefer a thread that does not block the signal (see Limits).

## Signals

Each running process owns a queue of pending process-directed signals — `Vec<PendingSignal>`, where each entry pairs the `Signal` with the sender's pid and an ABI-neutral `SignalSource` (mapped to an ABI `si_code` only when the information record is built) so a later record can be produced — a `SignalSet`
signal mask keyed per thread (each thread has its own mask; the main thread's is the process mask),
a per-thread targeted-pending map for `tgkill`/`SIGEV_THREAD_ID` signals, a `HashMap<Signal, SignalAction>` of configured dispositions, and a LIFO stack of
outstanding signal-frame addresses. These are empty when a process is constructed. Absence from
the action map means `Default`; installing `Ignore` removes already-pending instances of that
signal. Sending an ignored signal succeeds without queuing or waking the target. Otherwise
sending appends the signal (recording the current process as sender with `SignalSource::Process`)
while holding the process-table lock and wakes the target's main thread
after the lock is released. The sender never tears down the target directly: that target may still
execute on its own kernel stack. Signals whose effective default action is currently unsupported
are rejected before they enter this queue, while handler dispositions always queue. A masked
signal remains pending until the mask is replaced; `SIGKILL` and `SIGSTOP` cannot be masked,
ignored, or caught.

At a syscall return boundary, `deliver_pending_signal` removes the most recently queued unmasked
signal, resolves its disposition, and either executes the default action immediately or delivers
to a user handler. Handler delivery writes a signal frame below the interrupted user stack pointer
(skipping the 128-byte red zone and aligned to the System V entry convention), pushes the frame
address onto the process frame stack, adds the handler mask and the signal itself to the process
mask, and returns a `ResumeInfo` that the architecture layer applies to the saved user context.
The frame carries the trampoline entry as the handler return address, a snapshot of the
interrupted context, the pre-delivery mask, and the information record an `SA_SIGINFO` handler
reads. Its layout is a kernel-internal contract between `roxy-process` and the kernel-injected
trampoline. A plain handler observes only the signal number (its `RSI`/`RDX` are zeroed); an
`SA_SIGINFO` handler is invoked as `(signo, siginfo_t *, null)` with `RSI` pointing at the record
inside its own frame. Its third argument is null because this ABI serves no machine context for a
handler to inspect or redirect: the record that used to mirror the interrupted registers is gone,
and serving one would mean defining that state here and placing it on the frame.

The record is flat, one member per value, so every offset is a constant rather than an overlay
`si_code` selects. It carries the real `si_signo`, `si_code`, and sender `si_pid` recorded at queue
time, a timer's `si_value`, and — once a fault can raise a signal — `si_addr`. Its layout is
Roxy's own and it is defined next to the frame that writes it, not in the syscall subsystem that
also hands it to `sigtimedwait`; that subsystem's design records why it takes this record from
below rather than defining its own.
`pop_signal_frame` validates that the caller's stack pointer matches the recorded frame base
plus the popped return-address slot (the handler's `ret` consumes the frame's leading trampoline
address before the trampoline issues `sigreturn`), restores the context and mask, and is invoked
by the `sigreturn` syscall, which replaces the syscall-return contract itself in the syscall
subsystem. Spurious `sigreturn` calls return `EINVAL`; a handler that never returns (for example
after `longjmp`) leaks its frame entry, which is a known limitation of the single-frame-stack
model.

`execve` reverts all dispositions to `Default` and clears outstanding signal frames because
handler addresses point into the replaced image; the mask and pending set survive. The
terminating default action exits the current thread with a signal-derived `ExitStatus`; normal
`waitpid` reaping then reports that status as the Roxy wait record, which the libc renders as the
POSIX wait-status word its `WIFEXITED`/`WTERMSIG` macros decode. Delivery applies at
most one signal per userspace return boundary because termination does not return; remaining
pending signals are delivered at subsequent boundaries.

Fork clones the parent's dispositions while starting with no pending signals and no outstanding
signal frames.

## Initial descriptor injection

Initialization registers one `InitialFdInjector` before any process is spawned. Every direct
`spawn` creates a new empty descriptor table, invokes that injector, and publishes the process only
after injection completes. The injector is supplied by the composition root, so process owns the
creation sequence without depending on a terminal or hardware backend.

Fork does not invoke the injector: it clones the parent's open-file references. `execve` also does
not invoke it and preserves the current descriptor table. Closing an injected descriptor therefore
does not cause it to reappear. The current composition connects all three initial descriptors to
the kernel terminal selected by core and stored by the terminal subsystem; selecting separate
endpoints remains composition policy.

## Image and exec flow

Spawn and `execve` share the image builder:

```text
new AddrSpace → load executable → load PT_INTERP → map stack
→ encode startup stack → publish only after every step succeeds
```

`execve` first copies and validates all old-userspace arguments in the syscall layer. It then builds
the new image independently, replaces the process table's address space with interrupts disabled,
activates it, and returns the new entry/stack pair to the architecture layer. PID, main thread, and
FD table remain unchanged. A failed build leaves the old image untouched.

Fork snapshots its child return context before copying process-owned state. Address-space cloning can
traverse deeply into VM and allocator code, so the caller-provided register snapshot must not remain
only in transient ABI argument storage while that work runs. The child receives the preserved
context with a zero syscall result before it is published to the scheduler.

## Child wait flow

Waiting checks child ownership and exit state while holding the process-table lock. In the current
single-thread process model, a blocking wait registers one target for the parent process and
prepares its scheduler block before releasing that lock. Thread reaping takes the same lock before
publishing the child's `Exited` state and wakes the registered parent only when that child matches
its target, so unrelated child exits do not cause spurious wakeups and an exit cannot be lost
between the parent's check and block. The awakened parent rechecks state before reaping.

A successful wait removes exactly one zombie. Waiting for any child chooses the lowest exited PID
to keep selection deterministic without an additional exit-order queue. `WNOHANG` policy remains
at the syscall boundary; process reports whether a matching child is pending or absent.

## Lifecycle invariants

- A running process has a process-table entry, a thread-owner mapping, and an address space.
- A directly spawned process receives its completed initial descriptor table before publication.
- A directly spawned process starts in `/`; fork inherits cwd and `execve` does not change it.
- Address-space replacement is process-level; it is never performed by mutating a scheduler entry.
- A dying process retains its address space until its thread is safely reaped on another kernel
  stack.
- A child's parent ID remains stable through the parent's zombie state and becomes absent only when
  the parent is removed from the process table.
- Only a direct parent removes a child's exited entry, and each exited entry is returned once.
- Process-table inspection, waiter registration, scheduler block preparation, and exit publication
  share one lock order: process table before scheduler.
- A process starts exiting exactly once, when its first exiting thread marks it `Exiting`; its
  caller publishes the process-exit notification after releasing the process-table lock, so a
  handler may call process queries without re-entering the lock.
- The scheduler dispatch hook must activate the address space currently stored by the target
  process immediately before a user thread runs.

## Limits and non-goals

The current model supports multiple user threads sharing a process, with per-thread signal masks and
a per-thread targeted-pending queue, plus `tgkill` and `sigtimedwait` for thread-directed delivery
(enabling the libc's `SIGEV_THREAD`). `TODO(missing-capability: per-thread signal masks)` in
`table.rs`: process-directed delivery (`signal_target_thread`) still prefers the main thread and
otherwise any live thread rather than a thread that does not block the signal, so a process-wide
signal can queue against a thread whose mask blocks it instead of re-routing. A process-level exit
(`exit_current`) does not force-stop sibling threads before reaping, so a process whose main thread
exits while secondary threads remain can only be finalized when its last thread reaps (`TODO(missing-
capability: thread-teardown)` in `lifecycle.rs`). `execve` remains safe only from a single-threaded
process because it replaces the whole address space. There is no `FD_CLOEXEC` state,
so descriptors survive `execve`. ELF and existing `PT_INTERP` loading are supported; shebang
interpretation, multi-threaded exec cleanup, credentials, asynchronous interrupt-return delivery,
and PID 1 reparenting are not. POSIX real-time signals are supported; standard-signal
coalescing is not, so every delivery is queued and the most recent one is delivered first.
Process groups are tracked (`pgid`/`session_id`), with `setpgid`/`getpgid`/`setsid` and
foreground-group selection through `TIOCSPGRP`/`TIOCGPGRP`; session validation beyond membership
recording remains minimal (see `ISSUES.md`). Orphan zombies are retained because no init reaper
adopts them. Process-identity callers encode an absent parent as PID 0. `chdir` can replace cwd
after VFS validation; descriptor-based `fchdir` remains unsupported.
