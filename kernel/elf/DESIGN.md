# ELF Loader Design

## Purpose and scope

`roxy-elf` validates an in-memory ELF image and maps its loadable segments into a caller-owned
`AddrSpace`. It returns the entry point, load bias, program-header metadata, and optional
`PT_INTERP` path needed by process construction. It does not read files, resolve interpreters, or
construct a process startup stack.

## Responsibilities and ownership

The caller owns the address space and image bytes. The loader owns validation and the mapping
transaction: it must reject malformed metadata before exposing an unsafe mapping and must roll
back mappings when a later segment cannot be loaded.

`LoadType::Executable` accepts fixed-address `ET_EXEC` images with zero bias and PIE `ET_DYN`
images with the ELF subsystem's fixed executable base. `LoadType::Interpreter` accepts only
`ET_DYN` and uses the base address selected by the process image builder. The loader reports
metadata through repository types instead of exposing `object` crate types.

## Invariants

- Segment ranges are page-aligned and cannot overlap existing mappings.
- Segment permissions are derived from ELF flags and writable executable segments are rejected.
- The entry point must resolve to a valid mapped executable address.
- Program-header metadata and interpreter strings are preserved for startup auxiliary vectors.
- PIE entry points, segments, and program headers use the same checked load bias.
- Allocation and mapping failures are returned as `ElfError`; partial construction must not be
  published as a process image.

## Flow and limits

The loader parses headers, validates program-header and segment relationships, selects the
executable bias, maps zero-filled pages, copies file-backed bytes, and applies final permissions.
PIE currently uses a fixed base rather than ASLR. Only ELF loading is supported; shebang scripts,
relocations, symbol loading, and dynamic-linker policy belong elsewhere.
