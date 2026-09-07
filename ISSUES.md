# Known Issues

## ext4plus does not reclaim removed directories

`ext4plus 0.1.0-rc.2` can remove a directory entry without reclaiming the directory inode and its
allocated blocks. Repeated `rmdir` operations can therefore consume space until the volatile root
RAM disk is rebuilt at the next boot.

The adapter intentionally applies no reclamation workaround. The affected call site is marked with
a `FIXME` in `kernel/ext4/src/mutations.rs`.

## ext4plus final unlink of inline symlinks is unsafe

Short symbolic-link targets are stored inline in the inode. When their final directory entry is
unlinked, `ext4plus 0.1.0-rc.2` can interpret the inline target bytes as block pointers. This can
trigger an out-of-bounds block-group assertion or corrupt block accounting.

The adapter intentionally forwards the unlink without detection or a link-count workaround. The
affected call site is marked with a `FIXME` in `kernel/ext4/src/mutations.rs`.

## Foreground process groups exist but session validation is incomplete

Process groups, `setpgid`/`getpgid`/`setsid`, and TTY foreground-group selection
(`TIOCSPGRP`/`TIOCGPGRP`) are implemented, and Ctrl+C is delivered to the foreground group. When
no foreground group has been selected, the TTY falls back to the current reader. The session
model remains minimal: `setsid` skips the POSIX "caller must not already be a process group
leader" check because the spawn model makes every top-level process a leader, and `setpgid` does
not validate that target and group share a session. Both gaps are marked with `TODO(session)` in
`kernel/process/src/setpgid.rs`.

## pty and terminal semantics are only partially implemented

`roxy-tty-core`/`roxy-pty` do not yet implement several terminal behaviors. Each is marked with a
`TODO(<missing-capability>)` at its code site and described further in `kernel/pty/DESIGN.md`:

- `TODO(master-close-hangup)`: closing the last pty master does not signal EOF or `SIGHUP` to the
  slave, because `Device` has no per-open drop hook to detect it.
- `TODO(pty-lock)`: `TIOCSPTLCK` records the lock flag but a slave `open` does not yet reject a
  locked slave.
- `TODO(pty-gptpeer)`: `TIOCGPTPEER` is unsupported because the syscall layer cannot return a newly
  allocated descriptor from ioctl; callers open `/dev/pts/N` by number instead.
- `TODO(sigwinch)`: the process model has no `SIGWINCH`; master `TIOCSWINSZ` does not yet propagate
  to the slave.

## xtest aborts at the VFS root-mount test before most tests run

`cargo xtest` panics with "no current thread" in `kernel/thread/src/scheduler/state.rs`
(`current_thread_id`) inside the `kernel-main::hardcoded-root-device-is-mounted` test, aborting
before the rest of the distributed suite (including the `roxy-tty-core`/`roxy-pty` tests) runs. This
reproduced unchanged on a clean `HEAD`, so it is a pre-existing harness/ordering issue independent
of the terminal work; it blocks runtime validation of new tests but not `cargo xcheck` (format,
clippy, and both kernel builds).

## POSIX timer semantics are only partially implemented

`roxy-posix-timer` implements `timer_create`/`timer_settime`/`timer_gettime`/`timer_getoverrun`/
`timer_delete` for `SIGEV_NONE`, `SIGEV_SIGNAL`, and (via the libc) `SIGEV_THREAD`:

- The kernel timer ABI supports `SIGEV_NONE`/`SIGEV_SIGNAL` (process-directed) and
  `SIGEV_THREAD_ID` (`TimerNotify::SignalToThread`), which targets the timer signal at a specific
  thread's per-thread pending queue. `roxy-process` gained per-thread signal masks, a per-thread
  pending queue, `tgkill` delivery, and `sigtimedwait` to back thread-targeted timers.
- `SIGEV_THREAD` itself is implemented in the roxy mlibc (`sysdeps/roxy/time.cpp`) exactly as
  glibc does: `timer_create` spawns a helper pthread with the requested attributes; the helper
  blocks an internal realtime signal, publishes its kernel tid, and loops on `sigtimedwait`
  invoking `sigev_notify_function` on each expiration, backed by a kernel timer armed with
  `SIGEV_THREAD_ID` at that thread.

One POSIX behavior is knowingly approximated, marked with a `TODO(<missing-capability>)` at its
code site:

- `TODO(pending-aware-overrun)`: overrun counts expirations coalesced into a single delivered
  notification when the 250 Hz tick catches a timer up, rather than expirations missed while the
  previous expiration signal is still undelivered. Roxy has no pending-signal introspection.

The syscall surface and ABI records for these are in `kernel/syscall/src/syscalls/timer/`, and the
overrun approximation is documented in `kernel/posix-timer/DESIGN.md`.

## Syscall per-CPU state trusts userspace not to touch `GS`

The syscall entry resolves this CPU's kernel stack and user-`RSP` handoff through per-CPU
`GS`-relative storage (`GS.base` points at a `SyscallEntryState` slot). `CR4.FSGSBASE` stays clear so
userspace cannot run `wrgsbase`/`rdgsbase`, but a userspace program that loads a flat 64-bit data
selector into `GS` would zero the segment base and redirect the next syscall entry's `gs:` reads
to address zero (a kernel fault, not a privilege escalation). The kernel never uses `swapgs`, so
this is a deliberate no-swap, reserve-`GS` design. The supported userspaces (mlibc/Bash) never
touch `GS`, but hard hardening (conditional `swapgs` on ring-3 interrupt/exception entries, per
the Linux `SWAPGS_MASK` model) is future work. Documented in `kernel/arch/DESIGN.md`.

## Process threads: masks and targeted delivery are per-thread; teardown and exec single-thread

The process model attaches and reaps multiple user threads sharing one address space and
`descriptor table`, and now backs a real pthread implementation via the thread-create/exit/gettid
syscalls. Signal state is per-thread: each thread has its own mask and targeted-pending queue
(`pthread_sigmask` uses the `ThreadSigmask`/`SIGPROCMASK` thread-scope sysdep), and `tgkill` and
`sigtimedwait` are implemented (the latter enables `SIGEV_THREAD`'s helper thread). Remaining gaps,
marked with `TODO(<missing-capability>)` at their code sites in `kernel/process/`:

- `TODO(missing-capability: per-thread signal masks)` in `kernel/process/src/table.rs`:
  process-directed delivery (`signal_target_thread`) still prefers the main thread and otherwise any
  live thread rather than picking a thread that does not block the signal, so a process-wide signal
  can queue against a thread whose mask blocks it instead of re-routing.
- `TODO(missing-capability: thread-teardown)` in `kernel/process/src/lifecycle.rs`: a process-level
  exit (`exit_current`) sets `Exiting` but does not stop sibling threads, so a process whose main
  thread exits while secondary threads remain is only finalized when its last thread reaches the
  reap path on its own (no join, no `pthread_exit`, no forced sibling teardown).
- `execve` from a multi-threaded process is unsupported because it replaces the whole address space
  without quiescing other threads.

`kernel/process/DESIGN.md` documents the intended model.
