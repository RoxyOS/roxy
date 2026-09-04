# Pseudo-Terminal Design

## Purpose and scope

`roxy-pty` implements Unix pseudo-terminals: a master held by a terminal emulator (or `posix_openpt`
caller) paired with a slave that is the controlling terminal of a program. It allocates pairs,
exposes the master from `/dev/ptmx`, and exposes slaves from `/dev/pts/N`. It does not own keyboard
decoding, the line discipline, buffering, or terminal semantics — those live in `roxy-tty-core`;
`roxy-pty` only adapts one `TtyCore` between the pair's two byte streams and turns the pair into
character devices in `roxy-devfs`.

## Pair model

`PtyPair` connects two byte streams:

- `MasterOutput` (a queue plus poll listeners) is the slave's output/echo destination. The pair's
  `TtyCore` is constructed with it as the `TtyOutput`, so everything the slave writes (and echoes)
  accumulates here for the master to read.
- `SlaveInputSource` (a byte queue) feeds the `TtyCore`'s `TerminalInputSource`. It yields the
  stream one byte at a time so a newline reaches the line discipline as its own event and canonical
  mode can commit it. The `TtyCore` is constructed with it as the input source.

This is the mirror of a console terminal: where the console decodes keyboard events into bytes, the
pty master **writes** bytes into the slave's input; where the console draws output, the pty slave
**outputs** into the master's receive buffer.

## Master (`PtyMaster`, `impl Device`)

The master is not a terminal (`is_terminal` returns false): it is a "dumb" bidirectional pipe with
no line discipline. `read` drains `MasterOutput` and blocks on the architecture interrupt wait;
`write` pushes bytes into `SlaveInputSource` and then invokes the slave core's interrupt-time fast
path (`try_process_input_arrival`) followed by `observe_input`, so control characters like Ctrl+C
are processed and readers are woken. `poll`/`register_poll_listener` report or await master
readability from `MasterOutput`. `ioctl` answers `TIOCGPTN` (the pair number) and `TIOCSPTLCK`.

## Slave (`PtySlave`, `impl Device`)

The slave is a terminal (`is_terminal` returns true), observable through the descriptor layer's
`is_terminal` plumbing. Its `read`/`write`/`poll`/`register_poll_listener`/`ioctl` delegate
directly to the `TtyCore`, so the slave inherits line discipline, canonical editing, termios,
foreground groups, and controlling-session handling. The slave's `ioctl` goes straight to
`TtyCore::ioctl` (so `TIOCSCTTY` can make it a controlling terminal) and `write` reaches the master.

## Registry and devfs integration

`PtyRegistry` is a singleton that:

- Allocates a monotonically numbered `PtyPair` per `open`.
- Doubles as the `/dev/ptmx` **factory device**: `Device::open` returns a fresh `PtyMaster` for
  each open, so repeated `open("/dev/ptmx")` calls produce independent pairs.
- Doubles as the **dynamic resolver**: `DynamicDeviceResolver::resolve` maps `pts/N` to `PtySlave`.

kernel-main registers the singleton under `ptmx` and as the dynamic resolver. `DevFs` consults the
resolver only after the static table misses, so existing static devices are unaffected. Because the
VFS passes the full mount-relative path to `DevFs::open`, resolving `pts/N` does not require a
virtual `pts` directory node.

## Lifecycle, wakeups, and session handling

Slave writes wake the master reader via `MasterOutput`'s poll listeners; master writes wake the
slave reader via the slave core. Each pair's `TtyCore` is registered with the shared
session-leader-exit set in `roxy-tty-core`, so when a controlling session's leader exits, the slave
is released and its foreground group receives `SIGHUP` — the same path the console uses. Slaves
left open outlive their master reference because `PtySlave` holds the pair; a master closed while
no slave is open lets the pair drop.

## Limits

- `TODO(master-close-hangup)`: closing the last master does not yet signal EOF or `SIGHUP` to the
  slave, because `Device` has no per-open drop hook to detect it.
- `TODO(pty-lock)`: `TIOCSPTLCK` records the lock flag but slave `open` does not yet reject a locked
  slave.
- `TODO(pty-gptpeer)`: `TIOCGPTPEER` (open the peer from an fd) is not implemented; the syscall
  layer cannot yet return a newly allocated descriptor from ioctl. Users open `/dev/pts/N` by number
  instead.
- `TODO(sigwinch)`: master `TIOCSWINSZ` does not yet propagate to the slave or deliver `SIGWINCH`;
  the process model has no `SIGWINCH` delivery.
- Input/output transformations and non-default termios continue to be unsupported (rejected by
  `TtyCore`).