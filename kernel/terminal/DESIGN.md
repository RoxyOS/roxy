# Terminal Design

## Purpose and scope

`roxy-terminal` adapts shared terminal endpoints to the file-descriptor subsystem. It owns the
terminal endpoint contract and the common open-file behavior for terminals. It does not select the
active console, own hardware drivers, assign process descriptor numbers, or implement syscall ABI
policy.

Canonical input, echo, terminal attributes, pseudo-terminals, job control, and signal generation
are not implemented. They must extend this subsystem rather than introduce terminal-specific paths
in process or syscall code.

## Ownership and file adaptation

A `TerminalDevice` represents one shared, synchronized endpoint. Each terminal open creates an
independent `OpenFile` that holds an `Arc` to the endpoint. Descriptor closure therefore releases
only that open-file reference, while separate standard streams and processes may continue using the
same endpoint.

The adapter reports terminal identity, delegates metadata and byte I/O, and rejects seeking. It
does not interpret descriptor numbers or retain a file position. Concrete endpoints own their
blocking, buffering, output transformation, synchronization, and failure policies.

## Concurrency and extension contract

`TerminalDevice` is `Send + Sync` because independent open files may invoke one endpoint
concurrently. Implementations must serialize their mutable device state without holding unrelated
process or descriptor-table locks across I/O. Blocking reads must not retain locks needed by output
or kernel diagnostics while waiting.

Physical endpoints such as serial and `fbterm`, along with future PTY endpoints, implement the same
contract. Output-only endpoints emit the centralized unsupported-operation diagnostic before
returning their file error through the adapter. Terminal attributes and line discipline will
require explicit state and control interfaces in this subsystem; they must not be inferred from a
concrete backend or hidden inside syscall handlers.
