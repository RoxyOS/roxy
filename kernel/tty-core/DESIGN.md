# Terminal Core Design

## Purpose and scope

`roxy-tty-core` owns the byte-oriented semantics that every terminal — the console terminal and
each pty slave — shares: the line discipline, input buffering, blocking reads, output, terminal
ioctls, and foreground-process-group/session behavior. It is deliberately independent of any input
device or output hardware. Two narrow extension traits, [`crate::TerminalInputSource`] (where line
discipline input bytes come from) and [`crate::TtyOutput`] (where processed output and echo go),
keep it agnostic to whether the terminal is keyboard-driven (console) or fed by a pty master's
writes. It does not own keyboard decoding, hardware, filesystem device nodes, or syscall ABI.

`roxy-tty-core` sits between `roxy-line-discipline` (the byte policy) and `roxy-tty`/`roxy-pty`
(the terminal-endpoint users). It does not replace `roxy-tty-types`, which keeps the ABI-neutral
domain values (`Termios`, `WindowSize`, …) shared with `roxy-fd`.

## Ownership and behavior

`TtyCore` combines one `Arc<dyn TtyOutput>` endpoint, one `Arc<dyn TerminalInputSource>`, one
`LineDiscipline`, and the terminal-wide state: a readable byte buffer, a read lock, poll listeners,
the foreground process group, and the owning session. It is created through `TtyCore::new`, which
returns `Arc<Self>` and registers a weak reference with the process-wide session-leader-exit set.

### Input path

The environment injects input through [`TtyCore::process_input`], which runs one line-discipline
event and applies the result (buffer for reads, echo to the output endpoint, and any generated
signal to the foreground group). The blocking read path instead pulls from the input source via
`TerminalInputSource::next_input_bytes`, so committed input builds up without a caller needing to
inject it. A raised `TerminalInputSource` (for example a pty master write, or the console's
keyboard callback) calls `process_input` and then `observe_input` to wake a blocked reader.

`TtyCore::try_process_input_arrival` is the IRQ/callback-safe fast path that delivers VINTR
(Ctrl+C) or another control-character signal immediately even when no one is reading. It peeks the
source (`try_peek_bytes`), then acquires the line-discipline lock with `try_lock`; only if it holds
the discipline does it `consume_peeked` and process. This preserves the "pop an event only when both
locks are held" ordering so an input is never lost: on contention the input stays queued for the
read path.

### Read path

`TtyCore::read` enforces the foreground-read rule (`SIGTTIN` for background groups; `EIO` when
blocked/ignored), drains the readable buffer, pulls and processes input from the source while the
buffer is empty, returns `Interrupted` when a signal is pending, and otherwise blocks on an
architecture interrupt wait. `poll` uses the same under-lock filling and reports read readiness
from the buffer; output is always writable because terminal output has no backpressure model.

### Terminal attributes and ioctls

`TtyCore::ioctl` handles termios get/set with their application timing, window-size get/set,
`TIOCGPGRP`/`TIOCSPGRP` (with `SIGTTOU` for background callers), and `TIOCSCTTY`. `TCSAFLUSH`
discards the readable buffer, the discipline's partial line, and the input source's pending input
via `TerminalInputSource::discard_pending_input`. Unsupported fields are rejected with
`IoctlError::Unsupported`; pty master ioctls (`PtyGetNumber`, `PtySetLock`) are out of scope here
and reported `NotTty`. Other device ioctls (framebuffer, evdev) are likewise `NotTty`.

### Session and hangup semantics

`TtyCore::bind_session` makes a session leader's session the terminal's controlling session and
sets its foreground group; `TIOCSCTTY` does the same through ioctl. The foreground group receives
terminal-generated signals. Every live core registers a weak reference in a shared set; a single
process-side session-leader-exit handler (installed once by `TtyCore::new`) scans the set and, for
each core owned by the exited session, releases the terminal and sends `SIGHUP` to its foreground
group. This replaces the previous single-console exit handler and lets console and pty terminals
share one dispatch path.

A session leader that opens an unowned terminal acquires it as its controlling terminal without an
explicit `TIOCSCTTY` (Linux `tty_open` semantics): `TtyCore::try_acquire_controlling_terminal`
runs from devfs at terminal open, binds the caller's session and foreground group only when the
caller is a session leader, the session does not yet control a terminal, and this core is unowned
(a core can be bound at most once, via the same owner lock `bind_session`/`TIOCSCTTY` share).
`TtyCore` also serves the reverse lookup: `controlling_terminal_of(session)` scans the same live
weak set used by the exit handler to find a session's controlling terminal. The `/dev/tty` node is
a devfs dynamic resolver (`ControllingTerminalResolver`) in tty-core that maps the fixed `tty` path
to a `ControlTerminal` device wrapping the calling session's resolved core, or nothing (open fails)
when the process has no controlling terminal.

## Concurrency and extension contract

`TtyCore` and its extension traits are `Send + Sync`. The interrupt-time fast path runs with
interrupts disabled and uses `try_lock` on the line discipline; the read path uses full locks.
`TerminalInputSource` implementations document whether `next_input_bytes`/`try_peek_bytes`/
`consume_peeked`/`discard_pending_input` are IRQ-safe; the console source (a key-event deque) is,
and the pty source (a byte queue) is called from normal context. `TtyOutput::write` is called both
for echo (from apply paths) and for program output (`TtyCore::write`); endpoints must be `Send +
Sync` and serialize their mutable state.

A partial or failed echo returns an error without retrying and without dropping bytes already
moved into the readable buffer, matching the prior TTY behavior.

## Limits

Input/output transformations (`ONLCR` …), extra control characters, `VMIN`/`VTIME` combinations,
timeout-based noncanonical reads, job-control stop/continue, and `SIGWINCH` remain unsupported;
unsupported `termios` fields are rejected. The session model is shared with `roxy-process`'s
minimal `setsid`/`setpgid` checks. The registry keeps a weak set of all live cores rather than a
per-session index; an index is unnecessary while few terminals are live.