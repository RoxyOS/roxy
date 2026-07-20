# Serial Design

## Purpose and scope

`roxy-serial` owns the kernel's COM1 UART for its entire initialized lifetime. It provides normal
and emergency diagnostic output and exposes the UART as a generic terminal endpoint. It does not
assign process descriptors, own process creation policy, implement terminal line discipline, or
expose `uart_16550` types outside the subsystem.

The subsystem uses the existing `uart_16550` crate because it supplies the required `no_std` PIO
backend, UART configuration, byte send and receive operations, and typed 16550 register handling.

## Ownership and initialization

Initialization constructs COM1 once and stores it behind the subsystem lock. Normal diagnostics,
userspace terminal output, and receive polling all use this shared instance. Core initializes the
subsystem before architecture and process setup, then separately selects its terminal endpoint for
the initial-FD injector.

The `device` module owns the initialized UART object, its lock, and raw receive/send operations.
The `logging` module owns diagnostic entry points and reporter registration. The terminal adapter
owns blocking-read and LF-to-CRLF policy. This keeps hardware ownership independent from both
diagnostic and userspace terminal policy.

Emergency output first attempts the shared lock. If normal logging is unavailable while interrupts
are disabled, it constructs a temporary COM1 handle; this preserves panic diagnostics under the
same no-local-concurrency assumption as the architecture halt path.

## Terminal behavior and concurrency

The subsystem publishes one process-wide serial terminal instance. Each call to `terminal` clones
an `Arc` to that same endpoint, preserving stable terminal identity independently from the global
UART device state. Reads release the UART lock after each empty poll and halt until an interrupt
before retrying, so waiting input does not exclude diagnostics or output. The current architecture
has no external UART interrupt path, so the periodic timer bounds polling latency. Reads return
after at least one byte is available, and writes translate LF to CRLF while preserving all other
bytes.

The UART lock serializes hardware access across kernel logging and every process terminal. Multiple
readers compete for the same byte stream; selecting distinct physical terminals or PTYs remains a
core composition decision through the generic terminal interface.
