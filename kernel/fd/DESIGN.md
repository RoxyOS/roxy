# File Descriptor Design

## Purpose and scope

`roxy-fd` models process-local descriptor numbers, shared open-file state, file operations, and
the VFS file adapter. It also owns the typed ioctl requests dispatched to file objects. It does not
own a process table, syscall ABI request-number decoding, general syscall argument parsing, path
lookup, or the filesystem mount registry.

## Ownership model

`FdTable` owns descriptor-to-`Arc<OpenFile>` entries and allocates the lowest unused descriptor.
Cloning a table clones references to open files, which is the behavior required by fork. An
`OpenFile` owns a boxed `File` object and its current offset; its lock serializes operations that
must update the object and offset together. Directory objects hold an opening-time VFS snapshot and
interpret the shared offset as an entry index, so forked descriptors share directory iteration.

The `File` and `Directory` traits define object capabilities without implementing any concrete file
kind in this crate. Owning subsystems such as VFS and terminal implement those traits for their
objects. `File::as_directory` exposes optional directory iteration without concrete-type
downcasting, and `File::as_socket` exposes the same pattern for the socket-specific operations in
`SocketOps`; the FD layer does not identify objects by descriptor number or concrete backend type.
Socket operations run through `OpenFile::socket_ops` under the open-file lock, so blocking socket
operations hold that lock while waiting, matching blocking reads and writes.

The syscall boundary decodes personality-specific request numbers and layouts, copies pointer-based
ABI payloads, and constructs an `IoctlRequest` containing only layout-neutral kernel values or a
borrowed typed output. A getter fills that output directly during object dispatch, so the interface
cannot produce a response type that mismatches its request. `OpenFile::ioctl` holds the open-file
lock across this dispatch. `OpenFile::truncate` uses the same lock to serialize a length change
without changing the shared open-file offset. The `File` trait's default implementation returns
`IoctlError::NotTty`; file kinds override it only for supported requests.

Terminal-specific ioctl payload types belong to `roxy-tty-types`, which both this crate and
`roxy-tty` depend on. Framebuffer-specific payload types belong to `roxy-fb-types`, a
layout-neutral type crate with no device implementation, which `roxy-fbdev` and `roxy-syscall`
depend on directly. Both crates keep the descriptor layer free of device implementations.
`File::ioctl` embeds those layout-neutral domain values in one typed dispatch surface without
making the descriptor layer depend on a concrete device implementation. Userspace ABI layouts,
request numbers, errno policy, and raw userspace pointers remain exclusive to `roxy-syscall`.

## Invariants

- Descriptor numbers are valid only in the table that returned them.
- Reads, writes, directory iteration, seeks, and truncation serialize through one open file.
- Truncation changes the underlying object length without changing the shared open-file offset.
- Ioctl operations serialize against other operations on the same open file.
- A `File` receives only ioctl requests whose request-specific argument has already been decoded
  and copied out of userspace.
- FD types contain no userspace `#[repr(C)]` layout, padding, offset, request number, or raw user
  pointer; adding another ABI personality does not change the `File` interface.
- Terminal ioctl payload types are owned by `roxy-tty-types` and may not grow personality-specific
  semantics.
- Removing a descriptor drops one reference; the underlying object remains alive while other
  references or active VFS handles exist.
- Errors distinguish bad descriptors, unsupported operations, seekability, unconnected sockets,
  and underlying I/O failures at their owning layer.
- Stream-specific broken-pipe errors remain ABI-neutral until the syscall layer translates them
  into errno and signal behavior.

## Process interaction and limits

The process subsystem owns each `FdTable` and preserves it across `execve`, closing descriptors
marked close-on-exec. Each descriptor slot records whether it closes on `exec`; `open` sets the
flag from `O_CLOEXEC`. Fork copies the table's references rather than duplicating open-file
offsets, and the close-on-exec flag is inherited with the descriptor. `OpenFile::poll` exposes
ABI-neutral readiness for the syscall layer; each concrete object owns its readiness policy.
`File::register_poll_listener` is a separate operation that returns an RAII registration for
notification when readiness may have changed. The caller queries, registers, and prepares its
block with interrupts disabled, then rechecks readiness after any wakeup. VFS files and
directories report immediate read/write readiness, while devices such as the TTY derive it from
their state and register their listeners with a shared device queue. Duplication syscalls and
other per-descriptor flags remain unsupported.

The typed request set covers terminal attribute get/set operations with their application timing,
terminal window-size get/set operations, and framebuffer screen-information get operations.
`File::mmap` describes device physical memory for a file-backed `mmap`; only device-backed files
implement it, and the descriptor layer never installs or owns the mapping itself.
