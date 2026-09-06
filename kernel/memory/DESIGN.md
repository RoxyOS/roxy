# Physical Memory Design

## Purpose and scope

`roxy-memory` owns physical-frame allocation, the kernel heap, kernel page-table mapping, typed
physical/virtual/user addresses, and TLB/page-table primitives. It is the lowest memory layer
used by VM and ELF loading. It does not decide process allocation policy or parse executable files.

## Initialization flow

`initialize(BootInfo)` runs once: bootstrap heap setup, normalized memory-map construction, frame
allocator initialization with HHDM, kernel page-table backend initialization, kernel page-table
construction, and permanent heap setup. Repeated initialization or a boot map that leaves required
reserved regions usable is a kernel fault.

## Ownership model

`OwnedFrame` owns one allocated physical frame. `PageRef` represents a frame reference retained by
mapped pages. Page-table tokens and activation guards encode the lifetime of page-table state;
unsafe activation must keep the selected hierarchy alive until restoration or replacement.

The mapper owns kernel mappings and exposes a backend-neutral page-table API. It also maps
page-aligned device MMIO pages at their HHDM addresses when the bootloader did not include them in
the direct map. Device frames remain externally owned: the physical-frame allocator never owns,
poisons, or releases them. Architecture-specific mapper code owns CR3/page-table encoding. Address
newtypes validate canonicality and page alignment at the boundary rather than allowing raw integers
throughout the subsystem.

## Invariants

- Allocated frames are never handed out twice until their owner/reference count is released.
- User and kernel mappings use explicit permission flags; mapping rollback is required on partial
  construction failure.
- Kernel, bootloader, framebuffer, and other reserved physical ranges cannot be allocated as usable
  memory.
- Device MMIO mappings are writable, non-executable, and uncacheable; their physical frames are
  never transferred into allocator ownership.
- Page-table activation is only valid while the page-table hierarchy remains alive.

## Limits

The current backend is x86_64, and page-table and allocation code is not restricted to one CPU.
Memory statistics are diagnostic and must not become allocation policy. NUMA, demand paging,
swapping, and a per-CPU allocator remain outside this subsystem's current contract.
