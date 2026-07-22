# Time Design

## Purpose and scope

`roxy-time` provides global monotonic and realtime clocks plus the selected periodic timer backend.
It owns the accumulated monotonic duration, the Unix-time base captured at boot, timer calibration,
and tick duration. Interrupt entry, dispatch, and EOI belong to the interrupt subsystem.

## Clock model

The monotonic clock starts at zero and advances only through `advance(Duration)`. Updates saturate
instead of wrapping. Realtime is derived as:

```text
Unix seconds at boot + current monotonic duration
```

The boot realtime base is initialized once and is immutable afterward. Reading realtime before
initialization is a kernel-ordering error and panics.

The periodic timer backend is initialized separately after the local interrupt controller exists.
The current x86_64 backend programs the local APIC timer, calibrates it against PIT channel 2, and
registers a timer interrupt handler that advances monotonic time by one fixed tick.

## Concurrency and limits

The monotonic nanosecond counter is atomic and may be read without a lock. Backend initialization
and timer start require interrupts to be disabled. The current model assumes one authoritative BSP
periodic timer and does not correct drift, accept later wall-clock updates, track time zones, or
provide per-CPU clocks. Sleep and timeout queues belong to higher-level subsystems.
