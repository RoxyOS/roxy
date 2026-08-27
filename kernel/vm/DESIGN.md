# Virtual Memory Design

## Purpose and scope

`roxy-vm` owns user address spaces, user-region bookkeeping, anonymous allocations, user stacks,
mapping permissions, byte access, page-table activation, and eager fork copying. Physical frames
and low-level page tables are supplied by `roxy-memory`.

## Ownership model

`AddrSpace` owns one page-table hierarchy and maps each user page to a frame or guard state.
`AddrSpaceHandle` is a strong, locked reference used by process and syscall code. The process
subsystem is the long-lived owner of that handle; temporary clones support validated access and
fork construction.

Dropping an `AddrSpace` unmaps every mapped user page and releases retained frame references.
`AddrSpaceGuard` represents temporary activation and restores the previous page table on drop;
persistent process dispatch uses handle activation until another table is selected.

## Mapping invariants

- Every mapped page has matching bookkeeping and page-table state.
- Guard pages are reserved in bookkeeping but deliberately unmapped.
- Mapping a multi-page region either succeeds completely or rolls back completed pages.
- Anonymous free requires the exact original address and requested size; unmap rejects partial
  overlap with an allocation.
- Byte reads require mapped readable pages. Writes preflight the complete range and require every
  covered page to be writable, so a failing cross-page write cannot partially mutate memory.
- Permission changes cover a complete validated page-rounded range.

## Physical mappings

`AddrSpace` can map caller-owned physical memory directly into a user region through
`map_physical`, which requires a page-aligned physical base and page-rounded region. Each page is
recorded as `PageState::MappedPhysical` and unmapping, protecting, or dropping the address space
only removes the user page-table entries: the caller retains ownership of the physical pages,
which must remain valid for the mapping's lifetime. This is the device-mapping path used by
`mmap` of `/dev/fb0`.

Unmap accepts any page-aligned contiguous segment of one physical mapping, while anonymous
allocations must still match exactly. Fork shares physical mappings between the parent and child
address spaces (the shared-memory semantics of `MAP_SHARED`) instead of copying their contents;
anonymous pages remain eagerly copied.

## Lifecycle flows

ELF/process construction creates a mutable unpublished `AddrSpace`, maps segments and a guarded
stack, writes startup data, then converts it into a shared handle. Fork creates an eager private
copy of every mapped page and preserves guard/allocation metadata. `execve` constructs a new space
before replacing the process-owned handle.

## Limits

Fork is eager rather than copy-on-write. The implementation has no demand paging, swapping, shared
memory mappings, file-backed VM objects, or per-page fault recovery policy.

Physical mappings cover device memory only: there is no refcounted physical-page sharing with
frame-pool ownership transfer, and a physical mapping cannot outlive the caller's guarantee that
the physical range stays valid.
