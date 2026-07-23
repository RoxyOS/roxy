# TTY FD Adapter Design

## Purpose and scope

`roxy-ttyfd` combines one raw `InputDevice` and one `TerminalOutput` into the character-device
object exposed through process file descriptors. It owns the TTY's current fixed metadata and the
FD adaptation; it does not own hardware, rendering, line discipline, or syscall ABI parsing.

## Ownership and behavior

Initialization publishes one input/output pair before processes can receive their initial
descriptors. Each `open` creates a distinct `OpenFile` that retains `Arc` references to that pair.
Reads encode input events as UTF-8 characters or conventional terminal escape sequences, fill the
caller buffer, and wait with the architecture's atomic interrupt wait when no event is available.
Writes delegate directly to output, and seeks are rejected. The adapter defines the character-device
metadata so hardware backends do not need to know user-facing file identity or permissions.

## Limits

The adapter performs only event-to-byte encoding and forwards the resulting stream. Echo, canonical
mode, terminal attributes, ioctl handling,
PTYs, job control, and signals require a later line-discipline layer.
