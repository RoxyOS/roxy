# Boot Design

## Purpose and scope

`roxy-boot` converts bootloader responses into a bounded, owned `BootInfo` value used by kernel
initialization. The current loader is Limine. This subsystem does not initialize memory, mount the
root filesystem, start CPUs, or launch processes.

## Responsibilities and ownership

The loader backend owns the bootloader request statics and validates the responses it consumes.
`BootInfo` owns bounded copies of strings and metadata while module byte ranges remain borrowed
from bootloader-provided memory for the kernel lifetime.

The public data model normalizes memory-region kinds, framebuffer modes, kernel addresses, modules,
HHDM offset, the HHDM-mapped ACPI RSDP address, CPU identity, command line, and boot time. Downstream subsystems interpret this data;
they do not depend on Limine response types.

## Initialization flow

1. The bootloader satisfies the statically declared requests.
2. `BootInfo::parse` validates the supported revision, firmware, stack, and timestamp responses.
3. Limine responses are converted into bounded vectors and strings.
4. Kernel startup passes the resulting `BootInfo` to memory, time, rootfs, and interrupt-controller
   initialization. The interrupt subsystem uses the RSDP address through the HHDM mapping to parse
   the MADT; boot remains responsible only for exposing those normalized values.

Missing mandatory responses or an invalid environment are boot-fatal because later initialization
cannot establish its safety invariants.

Framebuffer metadata includes the RGB memory model and channel masks so `fbterm` can validate and
pack pixels without depending on Limine types. Mode compatibility remains a `fbterm` decision and
may fall back to serial during core initialization.

## Invariants and limits

All bounded collections have explicit capacities. Memory regions are reported with their original
classification, including unknown values, so memory initialization can reject unsafe assumptions.
The current backend requires EFI64 Limine. A missing framebuffer response produces an empty bounded
collection so core can retain its serial terminal. Other boot protocols must implement the sealed
`Bootloader` contract rather than bypassing `BootInfo`.
