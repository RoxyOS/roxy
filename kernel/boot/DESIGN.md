# Boot Design

## Purpose and scope

`roxy-boot` converts bootloader responses into a bounded, owned `BootInfo` value used by kernel
initialization. The current loader is Limine. This subsystem does not initialize memory, mount the
root filesystem, start CPUs, or launch processes.

## Responsibilities and ownership

The loader backend owns the bootloader request statics and validates the responses it consumes.
`BootInfo` owns bounded copies of strings and metadata while module byte ranges remain borrowed
from bootloader-provided memory for the kernel lifetime.

The public data model normalizes memory-region kinds, framebuffers, kernel addresses, modules,
HHDM offset, CPU identity, command line, and boot time. Downstream subsystems interpret this data;
they do not depend on Limine response types.

## Initialization flow

1. The bootloader satisfies the statically declared requests.
2. `BootInfo::parse` validates the supported revision, firmware, stack, and timestamp responses.
3. Limine responses are converted into bounded vectors and strings.
4. Kernel startup passes the resulting `BootInfo` to memory, time, and rootfs initialization.

Missing mandatory responses or an invalid environment are boot-fatal because later initialization
cannot establish its safety invariants.

## Invariants and limits

All bounded collections have explicit capacities. Memory regions are reported with their original
classification, including unknown values, so memory initialization can reject unsafe assumptions.
The current backend requires EFI64 Limine and a valid framebuffer response; other boot protocols
must implement the sealed `Bootloader` contract rather than bypassing `BootInfo`.
