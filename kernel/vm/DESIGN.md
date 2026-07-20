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
- Byte reads require mapped readable pages; writes additionally require writable permissions.
- Permission changes cover a complete validated page-rounded range.

## Lifecycle flows

ELF/process construction creates a mutable unpublished `AddrSpace`, maps segments and a guarded
stack, writes startup data, then converts it into a shared handle. Fork creates an eager private
copy of every mapped page and preserves guard/allocation metadata. `execve` constructs a new space
before replacing the process-owned handle.

## Limits

Fork is eager rather than copy-on-write. The implementation has no demand paging, swapping, shared
memory mappings, file-backed VM objects, or per-page fault recovery policy.
