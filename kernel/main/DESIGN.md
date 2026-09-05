# Kernel Main Design

## Purpose and scope

`kernel-main` is the composition root and executable kernel image. It establishes the initialization
order, initializes top-level exception, interrupt, and serial services, selects the kernel terminal,
starts the root filesystem, and selects normal boot or the kernel-test harness. Subsystems own their
services; the composition root should not duplicate their state or policy.

## Initialization contract

The startup sequence is intentionally ordered:

```text
clear BSS → serial → BootInfo → architecture → memory → select kernel terminal → time → rootfs
→ interrupt controller (ACPI MADT/IOAPIC) → CPU-local state → periodic timer backend → scheduler and POSIX-timer
handlers → PS/2 keyboard + mouse → TTY FD adapter → process → futex → syscall → start timer
→ enable interrupts → run/test
```

The composition root converts Limine's HHDM-mapped RSDP pointer back to a physical address, then
passes it and the HHDM offset to `roxy-interrupt`; it parses the MADT and configures IOAPIC routing
before any external line is unmasked.

Each subsystem must publish the global state required by later steps before the next step begins.
In particular, the periodic timer remains masked until time and scheduler handlers are registered,
process registers its scheduler address-space hook before user threads can run, and syscall
configures the architecture entry before interrupts are enabled.

PS/2 initialization follows scheduler registration and precedes both periodic timer startup and
global interrupt enable. The PS/2 subsystem completes its controller, keyboard, and mouse
handshakes, registers IRQ1 and IRQ12, and unmasks both routes in that window. The composition
root then combines the keyboard input with the selected terminal output through `roxy-tty`, which
creates the shared line discipline, registers the keyboard evdev device, the mouse evdev device,
and the keyboard and mouse listeners with their respective managers, before process initialization
registers the initial-FD injector. PS/2 keyboard hardware is required on the supported platform; a
missing controller or keyboard handshake timeout is boot-fatal rather than a reason to expose an
output-only framebuffer terminal. A missing or failed mouse is tolerated (the controller may have
no second port) and only logs a diagnostic message.

Timer handlers run in registration order. The time handler is registered before the scheduler and
POSIX-timer handlers so each periodic interrupt advances the monotonic clock before timer-wait
deadlines are evaluated, POSIX timers expire, and scheduler preemption is considered.

After memory initialization, normal builds initialize `fbterm` and select it as the kernel
terminal, falling back to serial after a serial diagnostic when the framebuffer mode is
unavailable. Kernel-test builds select serial directly so test progress remains visible on the
harness serial channel. The composition root passes the selected endpoint to `roxy-terminal`,
which owns its lifetime and ordinary formatted kernel output without owning this backend-selection
policy.

The root filesystem setup mounts the ext4 root, then mounts a `roxy-devfs` device filesystem at
`/dev` and publishes its shared `DeviceRegistry`. Immediately after the root filesystem is
initialized, the composition root asks `roxy-fbdev` to register the boot framebuffer; the device
appears only when `fbterm` published a validated layout, so serial-only boots expose no `fb0`.
This ordering keeps device registration before any userspace process can open `/dev` nodes while
leaving hardware and driver ownership in their subsystems.

Process initialization also receives the composition root's initial-FD injector. The current
injector creates three independent TTY open files at descriptors 0, 1, and 2 for every directly
spawned process. All three share the PS/2 input device, selected kernel output endpoint, and
TTY-level line discipline through `roxy-tty`. Process and syscall code remain unaware of the
selected backend. Descriptor assignment remains in `kernel-main` rather than the TTY or hardware
subsystems so a future composition may replace this initial stream policy.

## Ownership and failure

The composition root owns only composition-level handlers and the final kernel entry loop.
Boot-fatal setup errors panic with diagnostics; userspace operation failures remain inside their
owning subsystem and use the centralized unsupported-operation policy where required.

The panic handler reports through the serial subsystem's emergency path and halts or exits the test
kernel. The allocation error handler reports allocator and CPU statistics before panicking; it is
not a recoverable memory allocation path.

## Design limits

The current image assumes one statically selected init program and the existing Limine/x86_64 boot
path. Normal builds selecting `fbterm` encode PS/2 input events, process each complete encoded event
through the shared line discipline, and echo accepted input through the selected terminal. The
default discipline buffers input canonically until newline and consumes Backspace as an editing
operation. Kernel-test builds select serial for all three streams while still initializing required
PS/2 hardware and running its pure decoder and queue tests. Changes to initialization order, kernel
terminal selection, or initial descriptor policy must update this document and the affected
subsystem designs together.
