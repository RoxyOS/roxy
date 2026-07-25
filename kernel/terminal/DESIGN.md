# Terminal Design

## Purpose and scope

`roxy-terminal` defines shared terminal output endpoints and owns the kernel terminal selected by
the composition root. It does not choose a hardware backend, own hardware drivers, adapt endpoints
to file descriptors, assign process descriptor numbers, or implement syscall ABI policy.

Input devices, TTY file descriptors, line discipline, echo, terminal attributes, pseudo-terminals,
job control, and signal generation are outside this subsystem. `roxy-line-discipline` and
`roxy-tty` sit above raw input and output endpoints rather than introducing terminal-specific
paths in process or syscall code.

## Ownership and file adaptation

A `TerminalOutput` represents one shared, synchronized display endpoint. Concrete endpoints own
their buffering, output transformation, synchronization, display-size reporting, and failure
policies. `roxy-tty-types::WindowSize` is an ABI-neutral description of the endpoint's current
rows, columns, and pixel dimensions; endpoints without a physical window-size concept return zero
fields through `WindowSize::UNKNOWN`. User-facing
character device identity and file adaptation are owned by `roxy-tty`, so framebuffer and serial
output backends do not carry file metadata or read-side policy.

Every `TerminalOutput` implementation must explicitly report its window size. This makes a new
physical endpoint choose whether to expose its actual dimensions or deliberately return zero
fields; the trait provides no fallback that could hide an omitted implementation.

## Kernel terminal

Core selects exactly one `Arc<dyn TerminalOutput>` as the kernel terminal after the candidate
hardware endpoints are ready. The terminal subsystem stores that selection for the kernel lifetime;
selection is not a runtime-switching interface, and selecting twice or accessing it before selection
is a startup-contract violation. The subsystem does not know whether the selected endpoint is serial,
framebuffer-backed, or another future implementation.

Ordinary formatted kernel output uses the selected endpoint. The initial process descriptors use
`roxy-tty`, which combines this output endpoint with the platform input device.
Formatting is serialized under a preemption-disabling lock so all fragments produced by one
formatting operation reach the endpoint without interleaving with another ordinary kernel print on
the same or another CPU. Endpoint implementations still serialize their device state against
userspace writes. Panic, allocation-failure, exception, and mandatory unsupported-operation
diagnostics bypass the kernel terminal and retain their serial emergency or diagnostic paths so they
do not depend on the selected endpoint or its locks.

## Concurrency and extension contract

`TerminalOutput` is `Send + Sync` because kernel output and userspace writes may share one endpoint.
Implementations must serialize mutable display state without holding unrelated process or
descriptor-table locks across I/O or window-size queries. Physical endpoints such as serial and
`fbterm`, along with future display endpoints, implement the same output-and-size contract.
