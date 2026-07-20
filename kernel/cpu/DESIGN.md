# CPU Design

## Purpose and scope

`roxy-cpu` owns per-CPU initialization, local interrupt routing, timer setup, and CPU-local
statistics. It adapts architecture operations into kernel-facing CPU services. It does not own
thread scheduling, global time policy, or process state.

## Ownership and boundaries

`CpuLocal<T>` owns one initialized value for the current CPU. The current implementation exposes a
BSP-only model, so initialization and access assert that the caller is running on the BSP. The
architecture backend owns APIC and timer registers; this crate owns the lifecycle and statistics
that describe their use.

Initialization must occur with interrupts disabled and exactly once. It initializes the backend,
installs CPU-local counters, and starts the periodic timer. Interrupt handlers update counters and
delegate timer progress to `roxy-time` and scheduling to the thread subsystem.

## Invariants

- A CPU-local slot cannot be read before initialization or initialized twice.
- Interrupt depth and statistics are updated atomically.
- Timer handling must not run while the current CPU's interrupt state violates backend assumptions.
- Hardware identifiers are architecture values; public callers use the typed `CpuId` wrapper.

## Limits

SMP support is not represented by the current `CpuLocal` implementation. Extending it requires
replacing the BSP-only storage and revisiting lock, timer, and scheduler assumptions together.
