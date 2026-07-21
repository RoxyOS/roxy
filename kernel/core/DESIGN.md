# Kernel Core Design

## Purpose and scope

`roxy-kernel` is the composition root and executable kernel image. It establishes the initialization
order, initializes top-level exception, interrupt, and serial services, selects the initial process
terminal, starts the root filesystem, and selects normal boot or the kernel-test harness. Subsystems
own their services; core should not duplicate their state or policy.

## Initialization contract

The startup sequence is intentionally ordered:

```text
clear BSS → serial → BootInfo → architecture → memory → fbterm → time → rootfs
→ CPU → process → futex → syscall → enable interrupts → run/test
```

Each subsystem must publish the global state required by later steps before the next step begins.
In particular, process registers its scheduler address-space hook before user threads can run, and
syscall configures the architecture entry before interrupts are enabled.

Process initialization also receives core's initial-FD injector. The current injector creates three
independent terminal open files at descriptors 0, 1, and 2 for every directly spawned process. They
share the endpoint selected by core: a compatible `fbterm` framebuffer when available, or the
serial subsystem after a serial diagnostic for unsupported framebuffer modes. Process and syscall
code remain unaware of the selected backend. Descriptor assignment remains in core rather than the
hardware subsystems so other terminals or PTYs can replace the boot policy.

## Ownership and failure

Core owns only composition-level handlers and the final kernel entry loop. Boot-fatal setup errors
panic with diagnostics; userspace operation failures remain inside their owning subsystem and use
the centralized unsupported-operation policy where required.

The panic handler reports through the serial subsystem's emergency path and halts or exits the test
kernel. The allocation error handler reports allocator and CPU statistics before panicking; it is
not a recoverable memory allocation path.

## Design limits

The current image assumes one statically selected init program and the existing Limine/x86_64 boot
path. It does not provide framebuffer terminal input or serial input ownership between multiple
readers. Changes to initialization order or console selection must update this document and the
affected subsystem designs together.
