# Time Design

## Purpose and scope

`roxy-time` provides global monotonic and realtime clocks. It owns the accumulated monotonic
duration and the Unix-time base captured at boot. Hardware timer configuration and interrupt
delivery belong to the CPU subsystem.

## Clock model

The monotonic clock starts at zero and advances only through `advance(Duration)`. Updates saturate
instead of wrapping. Realtime is derived as:

```text
Unix seconds at boot + current monotonic duration
```

The boot realtime base is initialized once and is immutable afterward. Reading realtime before
initialization is a kernel-ordering error and panics.

## Concurrency and limits

The monotonic nanosecond counter is atomic and may be read without a lock. The current model assumes
one authoritative periodic timer and does not correct drift, accept later wall-clock updates, track
time zones, or provide per-CPU clocks. Sleep and timeout queues belong to higher-level subsystems.
