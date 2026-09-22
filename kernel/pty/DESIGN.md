# Pseudo-Terminal Design

## Purpose and scope

`roxy-pty` implements Unix pseudo-terminals as an **anonymous pair of file descriptions**: one
master that a terminal emulator holds and one slave that is the controlling terminal of a program.
A pair is allocated only through `open_pair()`, which the `ROXY_SYS_OPENPTY` syscall exposes to
userspace as libc's `openpty()`.

The crate does not own terminal semantics: keyboard decoding, the line discipline, buffering,
canonical editing, termios, foreground groups, and controlling-session handling all live in
`roxy-tty-core`; `roxy-pty` adapts one `TtyCore` between the pair's two byte streams.

Explicit non-goals:

- There is no device-filesystem node for a pair. `/dev/ptmx` and `/dev/pts/N` do not exist, so the
  Unix98 interfaces built on them — `posix_openpt`, `grantpt`, `unlockpt`, `ptsname`, `ptsname_r`,
  and the `TIOCGPTN`/`TIOCSPTLCK` ioctls — have no backing and no longer exist either.
- The crate does not name a pair. A slave has no reopenable path; its only handle is the descriptor
  `openpty()` returned.

## Pair model

`PtyPair` connects two byte streams:

- `MasterOutput` (a queue plus poll listeners) is the slave's output/echo destination. It is the
  `TtyOutput` of the pair's `TtyCore`, so everything the slave writes (and echoes) accumulates here
  for the master to read.
- `SlaveInputSource` (a byte queue) feeds the `TtyCore`'s `TerminalInputSource`. It yields the
  stream one byte at a time so a newline reaches the line discipline as its own event and canonical
  mode can commit it.

This mirrors a console terminal: where the console decodes keyboard events into bytes, the pty
master **writes** bytes into the slave's input; where the console draws output, the pty slave
**outputs** into the master's receive buffer.

## Master and slave (`impl File`)

Both ends are `roxy_fd::File` objects wrapped directly in `OpenFile`; they are never reached through
`roxy-devfs`, and the descriptor layer does not need a device handle to adapt them.

- `PtyMaster` is a "dumb" bidirectional pipe with no line discipline: `is_terminal` is false.
  `read` drains `MasterOutput` and blocks on the architecture interrupt wait, re-checking for a
  pending signal and returning `WouldBlock` under `O_NONBLOCK`; `write` pushes bytes into
  `SlaveInputSource` and then invokes the slave core's interrupt-time fast path
  (`try_process_input_arrival`) followed by `observe_input`, so control characters like Ctrl+C are
  processed and readers are woken. `poll`/`register_poll_listener` report or await master
  readability from `MasterOutput`. `ioctl` rejects every request with `NotTty`: the master carries
  no termios of its own, and `openpty` applies terminal attributes to the slave descriptor.
- `PtySlave` is a terminal: `is_terminal` is true. Its `read`/`write`/`poll`/
  `register_poll_listener`/`ioctl` delegate directly to the `TtyCore`, so the slave inherits line
  discipline, canonical editing, termios, foreground groups, and controlling-session handling. It
  reports no terminal pathname, so the terminal-name ioctl on a slave fails with `ENOTTY` and
  `ttyname()` cannot name it.

## Allocation and numbering

`open_pair()` allocates a `PtyPair` and returns `(master, slave)` as two `Arc<OpenFile>`. A
process-wide counter assigns each pair a number, used only to give the two descriptors distinct
`file_id` metadata; nothing looks a pair up by number. There is no registry: the pair is kept alive
solely by the two file descriptions that reference it.

## Lifecycle, wakeups, and session handling

Slave writes wake the master reader via `MasterOutput`'s poll listeners; master writes wake the
slave reader through the slave core. Each pair's `TtyCore` registers with the shared
session-leader-exit set in `roxy-tty-core`, so when a controlling session's leader exits the slave
is released and its foreground group receives `SIGHUP` — the same path the console uses. A slave
left open outlives its master reference because `PtySlave` holds the pair; when the last reference
drops, the pair drops with it.

Because allocation does not go through a device open, the pair does **not** acquire a controlling
terminal on creation. A program that wants the slave as its controlling terminal must call `setsid`
and then `TIOCSCTTY` on the slave descriptor (what libc's `login_tty`, and therefore `forkpty`,
does). This differs from a devfs terminal, which acquires on open.

## Limits

- `TODO(master-close-hangup)`: closing the last master does not yet signal EOF or `SIGHUP` to the
  slave, because the descriptor layer has no per-open drop hook to detect it.
- The master forwards no terminal ioctls, including `TIOCSWINSZ`; a window-size change must be
  applied to the slave descriptor. `TODO(sigwinch)`: the process model has no `SIGWINCH`, so a
  size change is not propagated or announced.
- A slave has no device path, so `ptsname`-style naming and any other reopen-by-name consumer cannot
  work. `ttyname()` on a slave reports `ENOTTY`.
- Attributes outside the implemented subset (`ICRNL`/`INLCR`/`IGNCR`, `OPOST`/`ONLCR`,
  `ISIG`/`ICANON`/`ECHO`, and the interrupt and erase characters) have no field in the terminal
  record, so the library drops them and a cooked terminal can still configure itself (see
  `ISSUES.md`).

## Rejected alternatives

- **Device-namespace pairs (Unix98 `/dev/ptmx` + `/dev/pts/N`).** The pair was previously a
  device-filesystem factory (`open("/dev/ptmx")` allocated a master) plus a dynamic resolver
  (`pts/N` resolved to the slave). That model forced a name namespace, a per-open factory hook in
  `roxy-devfs`, a `DynamicDeviceResolver` implementation, and the `TIOCGPTN`/`TIOCSPTLCK` ioctls
  whose only purpose was to let a caller reconstruct the slave path and reopen it. It also made
  allocation a two-step, racy sequence (read the number, then open the path). The anonymous fd pair
  replaces all of it: the caller receives the slave descriptor directly, so it needs neither a name
  nor a peer-open from an ioctl.

  The cost is compatibility: a Linux- or BSD-personality libc allocates a pty by opening
  `/dev/ptmx`, so those personalities cannot acquire a pty until a device path is reintroduced. That
  is recorded as a limitation in `ISSUES.md`.
