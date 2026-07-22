# Interrupt Design

## Purpose and scope

`roxy-interrupt` owns architecture-independent local interrupt dispatch and the selected local
interrupt controller backend. It tracks interrupt nesting, records delivery and controller-error
statistics, performs EOI for delivered local interrupts, and notifies all handlers registered for
the delivered local interrupt kind.

The architecture crate still owns interrupt entry stubs, IDT construction, vector numbers, and
interrupt-enable instructions. The time subsystem owns periodic timer-device configuration and
monotonic clock advancement. This subsystem does not own synchronous exception policy, external IRQ
routing, device drivers, timekeeping, or scheduler state.

## Ownership and boundaries

The x86_64 backend owns the local APIC operations needed for interrupt dispatch: software enabling,
error/spurious vector setup, EOI, and error-status reads. The time subsystem owns the APIC timer
programming path. Both access paths require interrupts to be disabled and are BSP-only in the
current model. Hardware types from `x2apic` and `x86_64` remain private to their owning backends.

`initialize` enables the local controller with timer delivery masked and returns the hardware CPU
identifier for `roxy-cpu`. Consumers register handlers with `register_local_handler` before
interrupts are enabled. Initialization installs the controller's APIC error and spurious-statistics
handlers in the same registry as time and scheduler consumers. Registration is append-only for the
boot lifetime and preserves call order.

## Interrupt flow and invariants

```text
architecture entry → interrupt dispatch → accounting/controller EOI → registered handlers
```

Dispatch updates interrupt nesting and statistics before notifying consumers. The dispatch guard is
dropped before handlers run because a handler may switch contexts. Timer and controller-error
interrupts send exactly one EOI; spurious interrupts update statistics without sending an EOI.

Initialization, registration, and controller access require interrupts to be disabled. Missing
initialization, duplicate handler registration, handler-list overflow, and unexpected interrupt
nesting are kernel faults.

## Extension points and limits

Each local interrupt kind currently has a fixed-size handler list sized for boot-time kernel
consumers. Adding external IRQs, handler removal, priorities, or dynamic device routing requires an
explicit routing contract rather than overloading this local-interrupt registry.
