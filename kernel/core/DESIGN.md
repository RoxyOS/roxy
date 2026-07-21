# Kernel Core Design

## Purpose and scope

`roxy-kernel` is the composition root and executable kernel image. It establishes the initialization
order, initializes top-level exception, interrupt, and serial services, selects the kernel terminal,
starts the root filesystem, and selects normal boot or the kernel-test harness. Subsystems own their
services; core should not duplicate their state or policy.

## Initialization contract

The startup sequence is intentionally ordered:

```text
clear BSS → serial → BootInfo → architecture → memory → select kernel terminal → time → rootfs
→ CPU → process → futex → syscall → enable interrupts → run/test
```

Each subsystem must publish the global state required by later steps before the next step begins.
In particular, process registers its scheduler address-space hook before user threads can run, and
syscall configures the architecture entry before interrupts are enabled.

After memory initialization, normal builds initialize `fbterm` and select it as the kernel terminal,
falling back to serial after a serial diagnostic when the framebuffer mode is unavailable. Kernel-test
builds select serial directly so test progress remains visible on the harness serial channel. Core
passes the selected endpoint to `roxy-terminal`, which owns its lifetime and ordinary formatted
kernel output without owning this backend-selection policy.

Process initialization also receives core's initial-FD injector. The current injector creates three
independent terminal open files at descriptors 0, 1, and 2 for every directly spawned process. All
three retain the selected kernel terminal endpoint. Process and syscall code remain unaware of the
selected backend. Descriptor assignment remains in core rather than the terminal or hardware
subsystems so a future composition may replace this initial stream policy.

## Ownership and failure

Core owns only composition-level handlers and the final kernel entry loop. Boot-fatal setup errors
panic with diagnostics; userspace operation failures remain inside their owning subsystem and use
the centralized unsupported-operation policy where required.

The panic handler reports through the serial subsystem's emergency path and halts or exits the test
kernel. The allocation error handler reports allocator and CPU statistics before panicking; it is
not a recoverable memory allocation path.

## Design limits

The current image assumes one statically selected init program and the existing Limine/x86_64 boot
path. Normal builds selecting `fbterm` therefore expose its unsupported input behavior through fd 0;
kernel-test builds select serial for all three streams. Changes to initialization order, kernel
terminal selection, or initial descriptor policy must update this document and the affected
subsystem designs together.
