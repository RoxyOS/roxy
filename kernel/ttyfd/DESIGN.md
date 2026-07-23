# TTY FD Adapter Design

## Purpose and scope

`roxy-ttyfd` combines one raw `InputDevice`, one `TerminalOutput`, and one `LineDiscipline` into the
character-device object exposed through process file descriptors. It owns the TTY's current fixed
metadata, line-discipline instance, and FD adaptation; it does not own hardware, rendering,
byte-processing policy, or syscall ABI parsing.

## Ownership and behavior

Initialization publishes one TTY before processes can receive their initial descriptors. The TTY
owns the input and output endpoints together with event-encoding progress, the line discipline,
pending results, and a TTY-level read lock. This makes input policy and partially processed bytes
terminal-wide rather than properties of whichever descriptor read first. Each `open` creates a
distinct `OpenFile` whose stateless `TtyFile` wrapper retains one `Arc` to the common TTY.

`Tty::read` encodes input events as UTF-8 characters or conventional terminal escape sequences and
passes each encoded byte to `LineDiscipline::process`. It applies the returned optional echo and
reader-delivery actions while holding the TTY read lock, so concurrent open files consume one
ordered stream. Echo completes before delivery. A failed or zero-length echo returns an I/O error
and retains the already processed result for the next read, so retry does not process the byte
twice. The lock is released before waiting with the architecture's atomic interrupt wait when no
event or readable result is available. `Tty::write` delegates directly to output; `TtyFile` only
adapts these operations, fixed metadata, and rejected seeks to the `File` interface.

The adapter defines the character-device metadata so hardware backends do not need to know
user-facing file identity or permissions.

## Limits

The current line discipline always delivers every encoded byte unchanged and echoes it when the
TTY's enabled-by-default termios echo setting requests that action. Canonical mode, userspace
termios configuration, other terminal attributes, ioctl handling, PTYs, job control, and signals
remain unsupported.
