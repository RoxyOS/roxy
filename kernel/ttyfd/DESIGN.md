# TTY FD Adapter Design

## Purpose and scope

`roxy-ttyfd` combines one raw `InputDevice`, one `TerminalOutput`, and one `LineDiscipline` into the
character-device object exposed through process file descriptors. It owns the TTY's current fixed
metadata, line-discipline instance, and FD adaptation; it does not own hardware, rendering,
byte-processing policy, or syscall ABI parsing.

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

Failed or partial echo returns an I/O error without discarding bytes already moved into the TTY
buffer; echo itself is not retried. The read lock is released before waiting with the architecture's
atomic interrupt wait. `Tty::write` delegates directly to output; `TtyFile` only adapts these
operations, fixed metadata, and rejected seeks to the `File` interface.

The adapter defines the character-device metadata so hardware backends do not need to know
user-facing file identity or permissions.

## Limits

The current line discipline supports fixed Backspace and newline canonical processing plus raw
event delivery when canonical mode is disabled. Userspace termios configuration, other control
characters and terminal attributes, ioctl handling, PTYs, job control, and signals remain
unsupported.
