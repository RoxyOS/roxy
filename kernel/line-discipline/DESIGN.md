# Line Discipline Design

## Purpose and scope

`roxy-line-discipline` owns a TTY's `Termios` state and the byte-level policy between its input
stream and the actions taken by its file-descriptor adapter. It does not own input devices, output
endpoints, file descriptors, waiting, or syscall policy, and it never calls back into
`roxy-ttyfd`.

## Processing contract

`LineDiscipline::process` accepts one byte and returns a `ProcessResult` with independent optional
bytes for reader delivery and echo. Returning actions keeps the subsystem independent from the
adapter that performs I/O and leaves filtering or transformation expressible without introducing
an output or descriptor dependency.

The initial policy always delivers every byte unchanged and includes the same byte as an echo
action only when `Termios::echo` is enabled. Echo defaults to enabled. UTF-8, control bytes, and
terminal escape sequences have no special handling. The owning TTY shares one discipline instance
and its settings across all open files and serializes mutable access before executing the returned
actions.

## Limits

Canonical input, editing, termios attributes other than echo, syscall configuration, signal
generation, input and output transformations, flushing, PTYs, and job control are outside the
current implementation.
