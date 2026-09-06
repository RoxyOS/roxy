# CPU Design

## Purpose and scope

`roxy-cpu` owns CPU identity, per-CPU initialization, and the `CpuLocal<T>` storage primitive. It
does not own interrupt routing, timer setup, interrupt statistics, thread scheduling, global time
policy, or process state.

## Ownership and boundaries

## Ownership and boundaries

`CpuLocal<T>` owns one initialized value for every CPU, stored in a fixed array indexed by the
typed `CpuId`. Initialization and access select the current CPU's slot through the architecture's
`current_cpu_id`, so there is no bootstrap-processor assertion inside the storage primitive. The
interrupt subsystem discovers the local APIC hardware identifier and passes it to CPU
initialization. Timer-device programming belongs to the time subsystem. AP identity is registered
by the architecture's application-processor bring-up, not by `roxy-cpu`.

Initialization must occur with interrupts disabled and exactly once. It installs the CPU-local
hardware identifier after `roxy-interrupt` has configured the local controller.

## Invariants

- A CPU-local slot cannot be read before initialization or initialized twice. Each slot is a
  `spin::Once`, which guarantees exactly one initialization and orders the value write before
  publish (release on completion, acquire on read), so a written value is never observed partially.
- Slot count is bounded by the exported `MAX_CPUS` constant in `roxy-arch`; `current_cpu_id` must
  stay below that bound or the access panics.
- Hardware identifiers are architecture values; public callers use the typed `CpuId` wrapper.
- The storage layer is per-CPU. Every slot is keyed by the real identity the architecture reports
  for the executing CPU; `current_cpu_id` resolves through the arch CPU map on both the bootstrap
  and application processors.

## Limits

`CpuLocal` storage supports one value per CPU. Non-bootstrap CPUs register through the
architecture CPU map during `Architecture::initialize_application_processor`, so slots beyond the
BSP are populated and `current_cpu_id` resolves a real per-CPU identity on every active CPU. Each
slot initializes exactly once with interrupts disabled (guarded by `spin::Once`, see the invariants above).
