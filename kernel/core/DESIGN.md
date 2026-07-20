# Kernel Core Design

## Purpose and scope

`roxy-kernel` is the composition root and executable kernel image. It establishes the initialization
order, installs top-level exception/interrupt/serial handlers, starts the root filesystem, and
selects normal boot or the kernel-test harness. Subsystems own their services; core should not
duplicate their state or policy.

## Initialization contract

The startup sequence is intentionally ordered:

```text
clear BSS → serial → BootInfo → architecture → memory → time → rootfs
→ CPU → process → futex → syscall → enable interrupts → run/test
```

Each subsystem must publish the global state required by later steps before the next step begins.
In particular, process registers its scheduler address-space hook before user threads can run, and
syscall configures the architecture entry before interrupts are enabled.

## Ownership and failure

Core owns only composition-level handlers and the final kernel entry loop. Boot-fatal setup errors
panic with diagnostics; userspace operation failures remain inside their owning subsystem and use
the centralized unsupported-operation policy where required.

The panic handler reports through serial output and halts or exits the test kernel. The allocation
error handler reports allocator and CPU statistics before panicking; it is not a recoverable memory
allocation path.

## Design limits

The current image assumes one statically selected init program and the existing Limine/x86_64
boot path. Changes to initialization order must update this document and the affected subsystem
designs together.
