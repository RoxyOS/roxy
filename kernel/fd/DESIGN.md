# File Descriptor Design

## Purpose and scope

`roxy-fd` models process-local descriptor numbers, shared open-file state, file operations, and
the VFS file adapter. It does not own a process table, syscall argument parsing, path lookup, or
the filesystem mount registry.

## Ownership model

`FdTable` owns descriptor-to-`Arc<OpenFile>` entries and allocates the lowest unused descriptor.
Cloning a table clones references to open files, which is the behavior required by fork. An
`OpenFile` owns a boxed `File` object and its current offset; its lock serializes operations that
must update the object and offset together.

The underlying `File` trait owns object-specific behavior. The VFS adapter translates a
`VfsFile`/`FileHandle` pair into that trait without exposing VFS implementation details to process
code. Other subsystems, including the terminal subsystem, adapt their objects through the same
trait; the FD layer does not identify terminals by descriptor number or concrete backend type.

## Invariants

- Descriptor numbers are valid only in the table that returned them.
- Reads, writes, and seeks serialize access to one open file's offset.
- Removing a descriptor drops one reference; the underlying object remains alive while other
  references or active VFS handles exist.
- Errors distinguish bad descriptors, unsupported operations, seekability, and underlying I/O
  failures at their owning layer.

## Process interaction and limits

The process subsystem owns each `FdTable` and preserves it across `execve`; there is currently no
`FD_CLOEXEC` state. Fork copies the table's references rather than duplicating open-file offsets.
The model is synchronous and does not yet include descriptor polling, duplication syscalls, or
per-descriptor flags.
