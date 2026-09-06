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
`timer_delete` for `SIGEV_NONE` and `SIGEV_SIGNAL`. Two POSIX behaviors are knowingly approximated
or absent, each marked with a `TODO(<missing-capability>)` at its code site:

- `TODO(pending-aware-overrun)`: overrun counts expirations coalesced into a single delivered
  notification when the 250 Hz tick catches a timer up, rather than expirations missed while the
  previous expiration signal is still undelivered. Roxy has no pending-signal introspection.
- `SIGEV_THREAD` and `SIGEV_THREAD_ID` are rejected with `EINVAL` in the roxy mlibc sysdeps
  (`timer_create`) because the process model has no per-thread signal delivery (`tgkill`).

The syscall surface and ABI records for these are in `kernel/syscall/src/syscalls/timer/`, and the
overrun approximation is documented in `kernel/posix-timer/DESIGN.md`.

## Syscall per-CPU state trusts userspace not to touch `GS`

The syscall entry resolves this CPU's kernel stack and user-`RSP` handoff through per-CPU
`GS`-relative storage (`GS.base` points at a `PerCpuSyscall` slot). `CR4.FSGSBASE` stays clear so
userspace cannot run `wrgsbase`/`rdgsbase`, but a userspace program that loads a flat 64-bit data
selector into `GS` would zero the segment base and redirect the next syscall entry's `gs:` reads
to address zero (a kernel fault, not a privilege escalation). The kernel never uses `swapgs`, so
this is a deliberate no-swap, reserve-`GS` design. The supported userspaces (mlibc/Bash) never
touch `GS`, but hard hardening (conditional `swapgs` on ring-3 interrupt/exception entries, per
the Linux `SWAPGS_MASK` model) is future work. Documented in `kernel/arch/DESIGN.md`.
