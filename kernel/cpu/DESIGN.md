# CPU Design

## Purpose and scope

`roxy-cpu` owns CPU identity, BSP-oriented CPU initialization, and the `CpuLocal<T>` storage
primitive. It does not own interrupt routing, timer setup, interrupt statistics, thread scheduling,
global time policy, or process state.

## Ownership and boundaries

`CpuLocal<T>` owns one initialized value for the current CPU. The current implementation exposes a
BSP-only model, so initialization and access assert that the caller is running on the BSP. The
interrupt subsystem discovers the local APIC hardware identifier and passes it to CPU
initialization. Timer-device programming belongs to the time subsystem.

Initialization must occur with interrupts disabled and exactly once. It installs the CPU-local
hardware identifier after `roxy-interrupt` has configured the local controller.

## Invariants

- A CPU-local slot cannot be read before initialization or initialized twice.
- Hardware identifiers are architecture values; public callers use the typed `CpuId` wrapper.

## Limits

SMP support is not represented by the current `CpuLocal` implementation. Extending it requires
replacing the BSP-only storage and revisiting lock, timer, and scheduler assumptions together.
