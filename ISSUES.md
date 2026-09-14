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
  slave, because the descriptor layer has no per-open drop hook to detect it.
- `TODO(sigwinch)`: the process model has no `SIGWINCH`; a slave window-size change is not
  propagated to the master or announced.

## Pseudo-terminals have no device-filesystem interface

A pty pair is allocated only by `openpty()` (`ROXY_SYS_OPENPTY`) and has no node under `/dev`.
`/dev/ptmx` and `/dev/pts/N` do not exist, and the Unix98 interfaces built on them —
`posix_openpt`, `grantpt`, `unlockpt`, `ptsname`, `ptsname_r`, `TIOCGPTN`, `TIOCSPTLCK` — have no
backing. The mlibc declarations of those functions remain (they are upstream code this fork does
not edit), but every call fails: `posix_openpt` cannot open `/dev/ptmx`, and `ptsname`/`unlockpt`
reach an unimplemented sysdep. A pty slave reports no `terminal_path`, so `ttyname()` on it returns
`ENOTTY` and no consumer can reopen it by name.

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

## Keyboard LEDs cannot be set

`/dev/keyboard` deliberately has no ioctl interface: the Roxy keyboard device serves its event
stream through `read` only. Keyboard LEDs (caps lock, num lock, scroll lock) therefore cannot be
driven from userspace, and the X keyboard driver's `SetLeds`/`GetLeds` are no-op implementations.

This is a deliberate capability gap rather than an oversight: adding LED control means adding a
control channel, which is a separate design decision from the event stream. A later revision can
introduce a small ioctl family with Roxy-owned request numbers, or a control record written back
to the device with `write`. The gap is marked with
`TODO(missing-capability: no ioctl channel for keyboard LEDs)` in the `xf86-input-keyboard` Roxy
backend patch and in `kernel/keyboard-dev/DESIGN.md`.

## Console output written while a client owns the framebuffer is lost from the display

`/dev/framebuffer`'s `TAKE_CONTROL` request suspends framebuffer terminal drawing so the kernel
cannot paint over a graphics client's pixels. The console has no cell grid or scrollback: the
rendered pixels are its only state, so output produced while drawing is suspended cannot be
repainted when the client releases the frame. Releasing therefore clears the screen and returns the
cursor to the home cell, which loses that output from the display (it remains on the serial
terminal and, for user programs, in the terminal's own buffered state).

The gap is marked with `TODO(missing-capability: console-text-model)` in
`kernel/fbterm/src/screen.rs`. A console that owns a cell grid and a redraw path could repaint its
own content on release, the way a Linux VT restores its text buffer, instead of clearing.

## The `open` flag word keeps Linux's numbering

Roxy numbers the flags it owns from a base above Linux's range, so a handler can tell another
personality's value from one of its own. The `open` flag word cannot: it is upstream mlibc's
`int`, Linux already uses its bits up to 25, and Roxy supports eight flags plus the two-bit access
mode, so no base above Linux's range fits beside them. The word therefore keeps Linux's numbering,
and `open.flags` reports an undefined bit as unsupported without being able to say whether it came
from Linux or from a caller asking for something of ours that does not exist.

The kernel reports the undefined bit rather than accepting it silently, which keeps the caller's
request visible; only the origin of the value is lost. The gap is marked with
`TODO(missing-capability: no owned numbering for the open flag word)` in
`kernel/syscall/src/syscalls/open.rs`. A wider word, or a request record Roxy defines itself, would
let the handler separate the two cases.

## `poll` ignores undefined event bits

`pollfd.events` is upstream mlibc's `short` and Linux already uses its bits up to 13, so the field
has no room for a base above Linux's range and keeps Linux's numbering. On top of that, `poll`
never inspects the requested bits at all: it reads the mask, answers from the descriptors it names,
and ignores everything else. A caller that asks for an event Roxy cannot report therefore waits
for something that will never wake it, and no diagnostic says so, which is the case the
centralized unsupported path exists to make visible.

The gap is marked with `TODO(missing-capability: no owned poll event word)` in
`kernel/syscall/src/syscalls/poll/mod.rs`. Widening the field, or taking the event list as a record
Roxy defines, would give the word a base above Linux's range and let `poll` report an undefined or
foreign bit through the same path as every other flag word in this subsystem.

## The timer clock and flag words keep Linux's numbering

`timer_create` takes a `clockid_t` and `timer_settime` a flag word, and Roxy numbers the values it
owns from a base above Linux's range. These two cannot: `CLOCK_REALTIME`, `CLOCK_MONOTONIC`, and
`TIMER_ABSTIME` are defined by upstream mlibc's `options/ansi/include/time.h`, which this fork does
not modify. The kernel therefore keeps Linux's numbering, and it reports a value it cannot serve
without being able to say whether it came from Linux or from a caller asking for something of ours
that does not exist. `timerfd`'s `TFD_TIMER_ABSTIME` has the same shape.

The kernel reports every value it cannot serve through the centralized diagnostic rather than
accepting it silently; only the origin of the value is lost. The gaps are marked with
`TODO(missing-capability: no owned numbering for the timer clock word)` and
`TODO(missing-capability: no owned numbering for the timer flags word)` in
`kernel/syscall/src/syscalls/timer/abi.rs`. Roxy-owned headers carrying those ids, or a `clockid_t`
the personality defines itself, would let the handlers separate the two cases.
