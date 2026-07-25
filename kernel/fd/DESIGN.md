# File Descriptor Design

## Purpose and scope

`roxy-fd` models process-local descriptor numbers, shared open-file state, file operations, and
the VFS file adapter. It also owns typed ioctl requests and the decoding of each request's number
and request-specific argument. It does not own a process table, general syscall argument parsing,
path lookup, or the filesystem mount registry.

## Ownership model

`FdTable` owns descriptor-to-`Arc<OpenFile>` entries and allocates the lowest unused descriptor.
Cloning a table clones references to open files, which is the behavior required by fork. An
`OpenFile` owns a boxed `File` object and its current offset; its lock serializes operations that
must update the object and offset together. Directory objects hold an opening-time VFS snapshot and
interpret the shared offset as an entry index, so forked descriptors share directory iteration.

The `File` and `Directory` traits define object capabilities without implementing any concrete file
kind in this crate. Owning subsystems such as VFS and terminal implement those traits for their
objects. `File::as_directory` exposes optional directory iteration without concrete-type
downcasting; the FD layer does not identify objects by descriptor number or concrete backend type.

`IoctlRequest` couples an operation with its decoded argument so file implementations never receive
an untyped request number and a separate raw argument. `OpenFile::ioctl` holds the open-file lock
while dispatching that complete request to the object. Unsupported raw request pairs fail during
`IoctlRequest::parse` before object dispatch. The `File` trait's default implementation returns
`IoctlError::NotTty`; file kinds override it only when they need to support a concrete request.

## Invariants

- Descriptor numbers are valid only in the table that returned them.
- Reads, writes, directory iteration, and seeks serialize access to one open file's offset.
- Ioctl operations serialize against other operations on the same open file.
- A `File` receives only ioctl requests whose request-specific argument has already been decoded.
- Removing a descriptor drops one reference; the underlying object remains alive while other
  references or active VFS handles exist.
- Errors distinguish bad descriptors, unsupported operations, seekability, and underlying I/O
  failures at their owning layer.

## Process interaction and limits

The process subsystem owns each `FdTable` and preserves it across `execve`; there is currently no
`FD_CLOEXEC` state. Fork copies the table's references rather than duplicating open-file offsets.
The model is synchronous and does not yet include descriptor polling, duplication syscalls, or
per-descriptor flags.

No ioctl request is currently implemented, so parsing every raw request and argument pair returns
`None`.
