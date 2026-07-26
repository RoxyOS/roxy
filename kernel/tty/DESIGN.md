# TTY Design

## Purpose and scope

`roxy-tty` combines one raw `InputDevice`, one `TerminalOutput`, and one `LineDiscipline` into the
character-device object exposed through process file descriptors. It owns the TTY's fixed file
metadata, mutable terminal settings and window size, line-discipline instance, and FD adaptation;
it does not own hardware, rendering, byte-processing policy, syscall ABI parsing, or the shared
TTY ioctl value types.

## Ownership and behavior

Initialization publishes one TTY before processes can receive their initial descriptors. The TTY
owns the input and output endpoints together with the line discipline, one readable byte buffer,
and a TTY-level read lock. This makes input policy, committed lines, and partially consumed data
terminal-wide rather than properties of whichever descriptor read first. Each `open` creates a
distinct `OpenFile` whose stateless `TtyFile` wrapper retains one `Arc` to the common TTY.

`Tty::read` first copies from its readable buffer. When that buffer is empty, it encodes one input
event as a UTF-8 character or conventional terminal escape sequence and passes the complete event
to `LineDiscipline::process`. Any result buffer is appended to the TTY buffer, while accepted echo
is written to the output endpoint. Canonical reads continue processing events until newline moves
a complete line from the discipline into the TTY buffer; noncanonical events move there
immediately. Concurrent open files consume one ordered stream under the TTY read lock.

`Tty::poll` uses the same lock and non-blocking input processing path to publish current
readability without entering the interrupt wait. A canonical TTY becomes readable only after a
complete line is committed; output is currently always writable because terminal output has no
backpressure model.

The TTY registers an input listener with its raw input device. Input arrival wakes its poll listener
queue, after which the syscall layer re-runs `Tty::poll`; a canonical partial line may therefore
produce a harmless wakeup but never a false readable result. Registration is separate from
readiness querying and is retained by an RAII guard for the duration of one blocked poll attempt.

Failed or partial echo returns an I/O error without discarding bytes already moved into the TTY
buffer; echo itself is not retried. The read lock is released before waiting with the architecture's
atomic interrupt wait. `Tty::write` delegates directly to output; `TtyFile` only adapts these
operations, fixed metadata, terminal ioctls, and rejected seeks to the `File` interface.

Terminal attribute ioctls expose the line discipline's `ECHO`, `ICANON`, and `VERASE` settings.
`roxy-tty-types` owns the shared terminal domain values; `roxy-fd` embeds them in its typed
`File::ioctl` surface without depending on this implementation. `roxy-tty` owns the behavior
behind those values, while `roxy-syscall` owns all userspace ABI layouts and request-number
translation.
Other fields have fixed values chosen so that applying mlibc's `cfmakeraw` to attributes returned
by this TTY changes only supported state: input and output flags are zero, character size is `CS8`,
`VMIN` is one, and speeds are zero. A setter that changes another field is rejected explicitly.
Switching from canonical to noncanonical mode makes a partial editing buffer readable. `TCSANOW`
and `TCSADRAIN` are equivalent because output writes are synchronous. `TCSAFLUSH` additionally
discards the readable buffer, partial canonical line, and currently queued input-device events.

Each TTY snapshots its initial window size from the selected `TerminalOutput`. Framebuffer-backed
terminals report their actual text grid and pixel dimensions; endpoints without a window-size
concept report zero fields. Window-size ioctls read or replace the TTY's shared state after
initialization. Changing it does not emit `SIGWINCH` because the current process model has neither
foreground process groups nor signal delivery; adding those facilities must connect notification
after the size update.

The adapter defines the character-device metadata so hardware backends do not need to know
user-facing file identity or permissions.

## Limits

The current line discipline supports configurable erase, newline canonical processing, and raw
event delivery when canonical mode is disabled. Input/output transformations, non-default speeds
and character sizes, `VMIN`/`VTIME` combinations, other control characters, PTYs, job control, and
signals remain unsupported.
