# Serial Design

## Purpose and scope

`roxy-serial` owns the kernel's COM1 UART for its entire initialized lifetime. It provides mandatory
and emergency diagnostic output and exposes the UART as a generic terminal output endpoint. It does
not assign process descriptors, own process creation policy, implement terminal line discipline,
provide userspace input, or expose `uart_16550` types outside the subsystem.

The subsystem uses the existing `uart_16550` crate because it supplies the required `no_std` PIO
backend, UART configuration, byte send and receive operations, and typed 16550 register handling.

## Ownership and initialization

Initialization constructs COM1 once and stores it behind the subsystem lock. Diagnostics, selected
kernel-terminal output, and userspace writes all use this shared instance when their composition
selects serial. Core initializes the subsystem before architecture and process
setup so serial is always available as a selection fallback and diagnostic path.

The `device` module owns the initialized UART object, its lock, and raw send operations. The
`logging` module owns diagnostic entry points and reporter registration. The terminal adapter owns
LF-to-CRLF output policy. This keeps hardware ownership independent from both diagnostic and
userspace TTY policy.

Emergency output first attempts the shared lock. If normal logging is unavailable while interrupts
are disabled, it constructs a temporary COM1 handle; this preserves panic diagnostics under the
same no-local-concurrency assumption as the architecture halt path.

## Terminal behavior and concurrency

The subsystem publishes one process-wide serial output instance. Each call to `terminal` clones an
`Arc` to that same endpoint. Writes translate LF to CRLF while preserving all other bytes. Serial
input is outside the current contract; the initial userspace TTY receives input from the platform
input device selected by core.

The UART lock serializes hardware access across diagnostics, kernel-terminal output, and every
process write routed through the selected TTY. Selecting distinct physical terminals or PTYs
remains a core composition decision above the generic output interface.
