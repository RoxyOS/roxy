# TTY Design

## Purpose and scope

`roxy-tty` is the **console terminal**: a thin, keyboard-driven user of `roxy-tty-core`. It turns
raw key events into bytes (a bounded pending key-event queue, the US 104-key layout decoder, and
the escape-sequence encoder) and feeds those bytes to a shared `TtyCore`, which owns the line
discipline, buffering, blocking reads, terminal ioctls, and foreground-group/session semantics.
`roxy-tty` keeps only the keyboard input bridge, the console singleton, controlling-terminal
binding, and the initial fd0/1/2 descriptors. It does not own termios semantics, the line
discipline, or the foreground-process-group machinery — a pty slave shares all of that through the
same core (see `roxy-tty-core/DESIGN.md` and `roxy-pty/DESIGN.md`).

## Ownership and behavior

Initialization publishes one console before processes can receive their initial descriptors. `Tty`
holds a `ConsoleInputSource` — the pending key-event queue plus decoder and encoder, exposing
`roxy_tty_core::TerminalInputSource` — around a `TtyCore`. The output endpoint is an
`Arc<dyn roxy_terminal::TerminalOutput>` adapted to `roxy_tty_core::TtyOutput`. Each `open` creates
a distinct `OpenFile` whose stateless `TtyFile` wrapper retains one `Arc` to the common `Tty` and
delegates reads, writes, poll, ioctl, and is-terminal to the core.

`initialize` returns the console as an `Arc<dyn roxy_keyboard_input::KeyboardListener>` that
`kernel/main` registers with the process-wide keyboard manager; it does not register with any
hardware driver. When the driver publishes a raw `KeyEvent`, the manager calls `on_recive_input` in
IRQ context.

### Input path

`on_recive_input` pushes the event onto the bounded `pending` queue, then runs the core's
IRQ-safe fast path (`TtyCore::try_process_input_arrival`) and wakes readers. The queue is sized to
the previous PS/2 driver queue depth and absorbs bursts the interrupt path cannot service.

The fast path makes `VINTR` (Ctrl+C) deliver `SIGINT` immediately even when nobody is reading. It
calls `ConsoleInputSource::try_peek_bytes`, which `try_lock`s the decoder (decoder first, matching
the read path's order) and decodes the front key without consuming it; only if the core then locks
the line discipline does it `consume_peeked` and process, so an event is popped only when both locks
are held and is never lost. Buffering and echo are best-effort at interrupt time.

`ConsoleInputSource::next_input_bytes` (used by the blocking read path) pops a key with interrupts
disabled, decodes it, and encodes the result as UTF-8 or a terminal escape sequence, skipping
no-output key releases. `Ctrl` is handled by the decoder's `MapLettersToUnicode` policy, so Ctrl+C
arrives at the line discipline as `\x03` and can trigger `ISIG`/`VINTR`.

### Read, write, poll, ioctl

These all delegate to `TtyCore`. `Tty::read`, `write`, `poll`, `register_poll_listener`, and
`ioctl` are pass-throughs; the shared semantics (blocking reads, `SIGTTIN`, canonical commit,
echo, termios/window-size/foreground-group ioctls, `TIOCSCTTY`) and the `TCSAFLUSH` pending-input
flush are described in `roxy-tty-core/DESIGN.md`. `TtyFile::is_terminal` returns true for the
console.

### Session handling

`roxy_tty::bind_controlling_terminal` binds the console to a session, and `kernel/main` calls it
for the init shell. When a controlling session's leader exits, the core's shared
session-leader-exit set releases the console and sends `SIGHUP` to its foreground group — the same
path a pty slave uses; there is no console-specific exit handler.

## Limits

The layout decoder is fixed to the US 104-key layout (`Us104Key`); layout selection and switching
are outside the current scope. Remaining terminal limits (unsupported `termios` fields, control
characters, job control, `SIGWINCH`, input/output transformations) live in
`roxy-tty-core/DESIGN.md` and apply to the console exactly as to a pty slave.