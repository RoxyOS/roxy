# Process Design

## Purpose and scope

`roxy-process` owns process identity, process state, the process table, process-owned address
spaces, descriptor tables, process creation, fork, `execve`, and process-facing virtual-memory
operations. It does not own scheduler context switching, ELF parsing policy, or VFS storage.

## Ownership and dependency boundaries

Each process owns exactly one optional `AddrSpaceHandle`, one main thread id in the current
single-thread model, and one `FdTable`. The process table maps thread ids to process ids so the
thread scheduler can request address-space activation without depending on this crate.

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

## Signals

Each running process owns `Vec<Signal>` collections for pending process-directed signals and its
signal mask, plus a `HashMap<Signal, SignalAction>` of configured dispositions. These are empty
when a process is constructed. Absence from the action map means `Default`; installing
`Ignore` removes already-pending instances of that signal. Sending an ignored signal succeeds
without queuing or waking the target. Otherwise sending appends the signal while holding the
process-table lock and wakes the target's main thread after the lock is released. The sender never
tears down the target directly: that target may still execute on its own kernel stack. Signals
whose effective default action is currently unsupported are rejected before they enter this queue.
A masked signal remains pending until the mask is replaced; `SIGKILL` and `SIGSTOP` cannot be
masked or ignored.

At a syscall return boundary, `process_latest_signal` removes the most recently queued signal of the
current process, resolves its disposition, and calls `process_signal`. Only `Default` may reach
delivery because ignored signals are discarded before queuing and installing `Ignore` clears
existing pending instances. The default disposition maps through `Signal::default_action`. A
terminating action exits the current thread with a signal-derived `ExitStatus`; normal
`waitpid` reaping then observes the corresponding low-byte signal status. This applies at most one
action because termination does not return.

Fork clones the parent's dispositions while starting with no pending signals. `execve` preserves
ignored dispositions. Future userspace-handler dispositions must instead reset to default during
successful image replacement.

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
- The scheduler dispatch hook must activate the address space currently stored by the target
  process immediately before a user thread runs.

## Limits and non-goals

The current model supports one thread per process and has no `FD_CLOEXEC` state, so descriptors
survive `execve`. ELF and existing `PT_INTERP` loading are supported; shebang interpretation,
multi-threaded exec cleanup, credentials, userspace signal handlers, asynchronous interrupt-return delivery,
process groups, and PID 1 reparenting are not. Process-owned signal-mask storage, atomic
block/unblock/replace operations, and pending delivery filtering are implemented. Consequently, a
process that never enters a syscall does not yet observe a pending terminating signal. Signal queues currently preserve
duplicate deliveries; it delivers the most recently queued signal first. POSIX standard-signal
coalescing is not implemented.
Orphan zombies are retained because no init reaper adopts them. Process-identity callers encode an
absent parent as PID 0. `chdir` can replace cwd after VFS validation; descriptor-based `fchdir`
remains unsupported.
