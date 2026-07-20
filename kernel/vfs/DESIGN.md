# Virtual Filesystem Design

## Purpose and scope

`roxy-vfs` provides normalized byte paths, filesystem and file-handle traits, mount routing, active
handle tracking, and global filesystem operations. It does not implement an on-disk filesystem or
own block devices.

## Paths and mount routing

`VfsPath` accepts absolute byte paths, rejects NUL and traversal above root, normalizes `.`/`..` and
repeated separators, and enforces path/component limits. Paths are bytes rather than UTF-8 because
the kernel ABI does not require filenames to be Unicode.

Mount resolution selects the longest matching component boundary. The resolved filesystem receives
a path local to its mount. A mount owns an `Arc<dyn FileSystem>` and active-handle counter; unmount
is rejected while any file from that mount remains active.

## Global interface and ownership

One `Vfs` is registered globally before process file operations begin. Free functions delegate to
that instance. Filesystems own persistent filesystem state, while `VfsFile` owns an active handle
and boxed `FileHandle` until close/drop.

Filesystem traits are synchronous and return `VfsError`. Filesystem-specific types and errors must
be translated inside adapters such as `roxy-ext4`.

## Invariants and limits

- Mount points are normalized and unique.
- Path resolution never matches a partial component such as `/mnt` against `/mnt2`.
- Filesystem callbacks are not invoked while the mount-table lock is held when that would allow
  re-entry or long I/O.
- Global registration occurs once; use before initialization is an explicit error.

The current VFS has no namespaces, per-process roots, permission credentials, page cache, or
asynchronous I/O.
