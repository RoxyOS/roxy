# Line Discipline Design

## Purpose and scope

`roxy-line-discipline` owns a TTY's `LineDisciplineSettings` and the byte-level policy between
its input stream and the actions taken by its file-descriptor adapter. It does not own input
devices, output endpoints, file descriptors, waiting, or syscall policy, and it never calls back
into `roxy-tty`.

## Processing contract

`LineDiscipline::process` accepts the bytes for one encoded input event atomically and returns a
`ProcessResult` containing an echo decision and optional readable buffer. Keeping event bytes
together prevents the bounded editing buffer from retaining only part of a UTF-8 character or
escape sequence. The discipline never performs output, descriptor operations, or reader delivery.

Echo and canonical input both default to enabled. In canonical mode, accepted ordinary events are
stored in the discipline's editing buffer and echoed immediately but remain unreadable until a
newline event commits the line. The configured erase byte defaults to `0x08`, removes the final
complete UTF-8 scalar, and is never committed; an empty buffer ignores it without echo. A committed
line includes its newline and is moved into the result's buffer, leaving the editing buffer empty
for the next line. Escape sequences otherwise remain ordinary buffered bytes.

The canonical buffer holds 4096 bytes and reserves its final byte for newline, so at most 4095
payload bytes are accepted. An event that would cross that boundary is rejected atomically and is
not echoed, while erase and newline remain available. Noncanonical mode returns each accepted event
immediately in the result buffer and does not interpret erase or newline. Changing from canonical
to noncanonical mode releases any partial line to the owning TTY, while an explicit input flush
discards it. The owning TTY shares one discipline instance and its settings across all open files.

## Limits

Control characters other than erase, EOF, line kill, signal generation, input and output
transformations, timeout-based noncanonical reads, PTYs, and job control are outside the current
implementation.
