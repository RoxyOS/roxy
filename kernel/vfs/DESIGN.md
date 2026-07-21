# Virtual Filesystem Design

## Purpose and scope

`roxy-vfs` provides normalized byte paths, filesystem and file-handle traits, mount routing, active
handle tracking, and global filesystem operations. It does not implement an on-disk filesystem or
own block devices.

## Paths and mount routing

`ResolvedPath` accepts absolute byte paths, rejects NUL and traversal above root, normalizes `.`/`..` and
repeated separators, and enforces path/component limits. It can also resolve a relative byte path
against an explicit absolute base; relative `..` remains at root when there is no parent. Paths are
bytes rather than UTF-8 because the kernel ABI does not require filenames to be Unicode.

The global VFS interface resolves every relative input through one registered working-directory
provider before dispatching the operation. The process subsystem registers that callback during
initialization and retains ownership of cwd state. Absolute paths bypass the callback. Provider
execution and path normalization complete before any filesystem or mount-table operation begins.

Mount resolution selects the longest matching component boundary. The resolved filesystem receives
a path local to its mount. A mount owns an `Arc<dyn FileSystem>` and active-handle counter; unmount
is rejected while any file from that mount remains active.

## Global interface and ownership

One `Vfs` is registered globally before process file operations begin. Free functions delegate to
that instance after applying the global path context. Filesystems own persistent filesystem state,
while `VfsFile` owns an active handle and boxed `FileHandle` until close/drop.

Filesystem traits are synchronous and return `VfsError`. Filesystem-specific types and errors must
be translated inside adapters such as `roxy-ext4`.

## Invariants and limits

- Mount points are normalized and unique.
- Filesystem operations and mount routing receive only normalized absolute paths.
- The working-directory provider may acquire the process-table lock, but it must not perform VFS
  operations or retain that lock after returning the cwd snapshot.
- Path resolution never matches a partial component such as `/mnt` against `/mnt2`.
- Filesystem callbacks are not invoked while the mount-table lock is held when that would allow
  re-entry or long I/O.
- Global registration occurs once; use before initialization is an explicit error.

The current VFS has no namespaces, per-process roots, permission credentials, page cache, or
asynchronous I/O. Relative global operations require a currently scheduled process after the
working-directory provider has been registered.
