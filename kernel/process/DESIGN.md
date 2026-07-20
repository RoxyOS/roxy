# Process Design

## Purpose and scope

`roxy-process` owns process identity, process state, the process table, process-owned address
spaces, descriptor tables, process creation, fork, `execve`, and process-facing virtual-memory
operations. It does not own scheduler context switching, ELF parsing policy, or VFS storage.

## Ownership and dependency boundaries

Each process owns exactly one optional `AddrSpaceHandle`, one main thread id in the current
single-thread model, and one `FdTable`. The process table maps thread ids to process ids so the
thread scheduler can request address-space activation without depending on this crate.

Each process also records an optional parent process ID. Directly spawned processes have no parent;
fork children record the caller's process ID. This relationship is informational and does not own
either process. An exited parent remains visible while its process-table entry is retained. Removing
that entry clears every child's matching parent ID, making the child an orphan.

The scheduler owns threads and saved contexts, but never owns a process address-space handle. The
ELF and VM crates provide construction primitives; process decides when a constructed image becomes
published.

## Initial descriptor injection

Initialization registers one `InitialFdInjector` before any process is spawned. Every direct
`spawn` creates a new empty descriptor table, invokes that injector, and publishes the process only
after injection completes. The injector is supplied by the composition root, so process owns the
creation sequence without depending on a terminal or hardware backend.

Fork does not invoke the injector: it clones the parent's open-file references. `execve` also does
not invoke it and preserves the current descriptor table. Closing an injected descriptor therefore
does not cause it to reappear. The current composition connects every directly spawned process to
one shared serial endpoint; selecting separate terminals remains composition policy.

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

## Lifecycle invariants

- A running process has a process-table entry, a thread-owner mapping, and an address space.
- A directly spawned process receives its completed initial descriptor table before publication.
- Address-space replacement is process-level; it is never performed by mutating a scheduler entry.
- A dying process retains its address space until its thread is safely reaped on another kernel
  stack.
- A child's parent ID remains stable through the parent's zombie state and becomes absent only when
  the parent is removed from the process table.
- The scheduler dispatch hook must activate the address space currently stored by the target
  process immediately before a user thread runs.

## Limits and non-goals

The current model supports one thread per process and has no `FD_CLOEXEC` state, so descriptors
survive `execve`. ELF and existing `PT_INTERP` loading are supported; shebang interpretation,
multi-threaded exec cleanup, credentials, signals, process groups, PID 1 reparenting, and child
process collections are not. Process-identity callers encode an absent parent as PID 0.
