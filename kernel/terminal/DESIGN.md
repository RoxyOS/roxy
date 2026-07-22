# Terminal Design

## Purpose and scope

`roxy-terminal` defines shared terminal endpoints, adapts them to the file-descriptor subsystem,
and owns the kernel terminal selected by the composition root. It does not choose a hardware
backend, own hardware drivers, assign process descriptor numbers, or implement syscall ABI policy.

Canonical input, echo, terminal attributes, pseudo-terminals, job control, and signal generation
are not implemented. Physical endpoints expose raw byte streams; for the framebuffer endpoint this
is the ASCII stream supplied by `roxy-ps2`. Future line discipline must extend this subsystem
rather than introduce terminal-specific paths in process or syscall code.

## Ownership and file adaptation

A `TerminalDevice` represents one shared, synchronized endpoint. Each terminal open creates an
independent `OpenFile` that holds an `Arc` to the endpoint. Descriptor closure therefore releases
only that open-file reference, while separate standard streams and processes may continue using the
same endpoint.

The adapter reports terminal identity, delegates metadata and byte I/O, and rejects seeking. It
does not interpret descriptor numbers or retain a file position. Concrete endpoints own their
blocking, buffering, output transformation, synchronization, and failure policies.

## Kernel terminal

Core selects exactly one `Arc<dyn TerminalDevice>` as the kernel terminal after the candidate
hardware endpoints are ready. The terminal subsystem stores that selection for the kernel lifetime;
selection is not a runtime-switching interface, and selecting twice or accessing it before selection
is a startup-contract violation. The subsystem does not know whether the selected endpoint is serial,
framebuffer-backed, or another future implementation.

Ordinary formatted kernel output and the initial process descriptors use the selected endpoint.
Formatting is serialized under a preemption-disabling lock so all fragments produced by one
formatting operation reach the endpoint without interleaving with another ordinary kernel print on
the same or another CPU. Endpoint implementations still serialize their device state against
userspace writes. Panic, allocation-failure, exception, and mandatory unsupported-operation
diagnostics bypass the kernel terminal and retain their serial emergency or diagnostic paths so they
do not depend on the selected endpoint or its locks.

## Concurrency and extension contract

`TerminalDevice` is `Send + Sync` because independent open files may invoke one endpoint
concurrently. Implementations must serialize their mutable device state without holding unrelated
process or descriptor-table locks across I/O. Blocking reads must not retain locks needed by output
or kernel diagnostics while waiting.

Physical endpoints such as serial and `fbterm`, along with future PTY endpoints, implement the same
contract. A raw input endpoint may return any available byte count and wait only when its queue is
empty; `fbterm` polls after interrupt-driven CPU halts rather than publishing scheduler waiters.
Waiting must not hold output, process, or descriptor locks.
Output-only endpoints emit the centralized unsupported-operation diagnostic before returning their
file error through the adapter. Terminal attributes and line discipline will require explicit state
and control interfaces in this subsystem; they must not be inferred from a concrete backend or
hidden inside syscall handlers.
