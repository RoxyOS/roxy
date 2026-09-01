# TTY Design

## Purpose and scope

`roxy-tty` combines one raw input listener, one `TerminalOutput`, and one `LineDiscipline` into
the character-device object exposed through process file descriptors. It owns the TTY's fixed
file metadata, mutable terminal settings and window size, line-discipline instance, FD
adaptation, and the keyboard layout decoder that turns raw key events into characters; it does
not own hardware, rendering, syscall ABI parsing, or the shared TTY ioctl value types.

## Ownership and behavior

Initialization publishes one TTY before processes can receive their initial descriptors. The TTY
owns the output endpoint together with the line discipline, the layout decoder, a bounded queue
of pending raw key events, one readable byte buffer, and a TTY-level read lock. This makes input
policy, committed lines, and partially consumed data terminal-wide rather than properties of
whichever descriptor read first. Each `open` creates a distinct `OpenFile` whose stateless
`TtyFile` wrapper retains one `Arc` to the common TTY.

`initialize` returns the TTY as an `Arc<dyn roxy_input::InputListener>`. `kernel/main` registers
it with the process-wide input manager (`roxy_input::register_listener`); it does not register
with any hardware driver. When the driver publishes a raw `KeyEvent`, the input manager calls the
TTY's `on_recive_input(key)` in IRQ context.

### Input path

`on_recive_input` pushes the event onto the bounded `pending` queue, then tries to process it
immediately in interrupt context, then wakes poll listeners. The queue is sized to the previous
PS/2 driver queue depth and absorbs bursts that the interrupt path cannot service (its locks are
held by the interrupted thread).

The interrupt-time processing path makes `VINTR` (Ctrl+C) deliver `SIGINT` immediately even when
nobody is reading the TTY (e.g. a foreground child is running). It acquires the decoder and
line-discipline locks with `try_lock` in that fixed order (decoder first, matching the read
path's acquisition order) and pops an event from `pending` only after both locks are held; a
failed `try_lock` leaves the event queued for the read path. A decoded result signal is
delivered to the foreground process group (or, without one, to the current reader when it can be
resolved in IRQ context). Buffering and echo use separate locks and are best-effort at interrupt
time.

### Read path

`Tty::read` first copies from its readable buffer. When that buffer is empty, it pops one raw
`KeyEvent` from `pending`, feeds it through the decoder (which updates modifier state), encodes
the resulting character or special key as UTF-8 bytes or a terminal escape sequence, and passes
the complete event to `LineDiscipline::process`. Any result buffer is appended to the TTY
buffer, while accepted echo is written to the output endpoint. A result signal is delivered to
the current process via `roxy_process::send_signal`; a subsequent loop iteration observes the
pending signal and returns `EINTR`, letting signal delivery happen at the userspace return
boundary. Canonical reads continue processing events until newline moves a complete line from the
discipline into the TTY buffer; noncanonical events move there immediately. Concurrent open files
consume one ordered stream under the TTY read lock.

Releases of non-modifier keys yield `None` from the decoder and produce no bytes; modifier
releases update decoder state only. `Ctrl` is handled by the decoder's `MapLettersToUnicode`
policy, so Ctrl+C arrives at the line discipline as `\x03` and can trigger `ISIG`/`VINTR` signal
generation.

`Tty::poll` uses the same lock and non-blocking input processing path to publish current
readability without entering the interrupt wait. A canonical TTY becomes readable only after a
complete line is committed; output is currently always writable because terminal output has no
backpressure model.

Input arrival wakes its poll listener queue, after which the syscall layer re-runs `Tty::poll`; a
canonical partial line may therefore produce a harmless wakeup but never a false readable result.
Registration with the input manager is separate from readiness querying and is retained by an
RAII guard for the duration of one blocked poll attempt.

### Locking and IRQ safety

The `pending` queue lock is accessed from both the IRQ path (push/pop) and the read path (pop).
The IRQ path runs with interrupts disabled, so it takes the queue lock directly. The read path
and ioctl flush disable interrupts (`CurrentArchitectureBackend::without_interrupts`) while
holding the queue lock, so the IRQ path never contends with a thread that owns it. The decoder
and line-discipline locks are always taken in decoder-first order; the IRQ path uses `try_lock`
for both.

Failed or partial echo returns an I/O error without discarding bytes already moved into the TTY
buffer; echo itself is not retried. The read lock is released before waiting with the
architecture's atomic interrupt wait. `Tty::write` delegates directly to output; `TtyFile` only
adapts these operations, fixed metadata, terminal ioctls, and rejected seeks to the `File`
interface.

Terminal attribute ioctls expose the line discipline's `ECHO`, `ICANON`, `ISIG`, `VERASE`, and
`VINTR` settings. `roxy-tty-types` owns the shared terminal domain values; `roxy-fd` embeds them
in its typed `File::ioctl` surface without depending on this implementation. `roxy-tty` owns the
behavior behind those values, while `roxy-syscall` owns all userspace ABI layouts and request-number
translation.
Other fields have fixed values chosen so that applying mlibc's `cfmakeraw` to attributes returned
by this TTY changes only supported state: input and output flags are zero, character size is `CS8`,
`VMIN` is one, and speeds are zero. A setter that changes another field is rejected explicitly.
Switching from canonical to noncanonical mode makes a partial editing buffer readable. `TCSANOW`
and `TCSADRAIN` are equivalent because output writes are synchronous. `TCSAFLUSH` additionally
discards the readable buffer, partial canonical line, and the pending raw-event queue.

Each TTY snapshots its initial window size from the selected `TerminalOutput`. Framebuffer-backed
terminals report their actual text grid and pixel dimensions; endpoints without a window-size
concept report zero fields. Window-size ioctls read or replace the TTY's shared state after
initialization. Changing it does not emit `SIGWINCH` because the current process model has no
`SIGWINCH` delivery; adding it must connect notification after the size update.

The adapter defines the character-device metadata so hardware backends do not need to know
user-facing file identity or permissions.

## Limits

The layout decoder is fixed to the US 104-key layout (`Us104Key`); layout selection and
switching are outside the current scope. The current line discipline supports configurable erase,
newline canonical processing, raw event delivery when canonical mode is disabled, and
`ISIG`/`VINTR` interrupt-character signal generation. The TTY tracks a foreground process group
selected through `TIOCSPGRP`/`TIOCGPGRP`; when one is selected, terminal-generated signals are
delivered to every process in it, and the blocked reader returns `EINTR` so delivery happens at
the userspace return boundary. Without a selected group the TTY falls back to the current reader.
Input/output transformations, non-default speeds and character sizes, `VMIN`/`VTIME` combinations,
other control characters, PTYs, job control, and other signal generation remain unsupported.
