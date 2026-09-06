# Interrupt Design

## Purpose and scope

`roxy-interrupt` owns architecture-independent interrupt dispatch and the selected APIC/IOAPIC
controller backend. It tracks interrupt nesting, records local-controller and IRQ statistics,
performs EOI, and notifies all handlers registered for the delivered event.

The architecture crate still owns interrupt entry stubs, IDT construction, vector numbers, and
interrupt-enable instructions. The time subsystem owns periodic timer-device configuration and
monotonic clock advancement. This subsystem does not own synchronous exception policy, device
drivers, timekeeping, or scheduler state.

## Ownership and boundaries

The x86_64 backend owns local APIC operations and parses the ACPI MADT to configure the first IOAPIC
covering ISA IRQ0..IRQ15. The MADT's reserved physical IOAPIC page is mapped through
`roxy_memory::map_mmio` before `x2apic::ioapic::IoApic` is constructed; the interrupt backend does
not assume that Limine's HHDM includes device MMIO. Redirection entries use fixed vectors, physical
BSP delivery, edge/high polarity, and remain masked until a consumer unmasks a line. The legacy PIC
is masked when APIC mode is selected. Hardware types from `acpi`, `x2apic`, and `x86_64` remain
private to this backend.

`initialize` enables the local controller with timer delivery masked and returns the hardware CPU
identifier for `roxy-cpu`. `initialize_ap` does the same for each application processor but skips
the BSP-only IOAPIC/dispatcher setup (the BSP owns those once). Consumers register local handlers
with `register_local_handler` and
external handlers with `register_irq_handler` before interrupts are enabled. Initialization installs
the controller's APIC error and spurious-statistics handlers in the same registry as time and
scheduler consumers. Registration is append-only for the boot lifetime and preserves call order.

A reschedule IPI uses its own local vector (`LocalInterruptKind::Reschedule`); `send_reschedule_ipi`
targets a logical CPU by translating back through the arch CPU map. The target needs no registered
handler: delivery alone wakes it out of the scheduler's `wait_for_interrupt`, after which the
interrupt is EOI'd and the target re-enters its dispatch loop.

## Interrupt flow and invariants

```text
architecture entry → interrupt dispatch → registry handlers → accounting/controller EOI
```

Local dispatch updates nesting and sends EOI before notifying consumers because a local handler may
switch contexts. External IRQ handlers run while the dispatch guard is held; after all handlers
return, one EOI is sent. Spurious interrupts update statistics without sending an EOI.

Initialization, registration, and controller access require interrupts to be disabled. Missing
initialization, duplicate handler registration, handler-list overflow, and unexpected interrupt
nesting are kernel faults.

## Extension points and limits

Each local kind and ISA IRQ line has a fixed-size handler list sized for boot-time kernel consumers.
IRQ handlers must acknowledge their device without blocking, switching threads, or re-enabling
interrupts. IRQ1 and IRQ12 are ready for PS/2 registration, but device drivers remain outside this
subsystem. Interrupt Source Overrides affecting those lines are rejected until routing policy exists.
