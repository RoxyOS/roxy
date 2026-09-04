# Virtual Filesystem Design

## Purpose and scope

`roxy-vfs` provides normalized byte paths, filesystem, file and directory handles, mount routing,
active-handle tracking, and global filesystem operations. It does not implement an on-disk
filesystem or own block devices.

## Paths and mount routing

`ResolvedPath` represents only a normalized absolute byte path. `ResolvedPath::resolve` is the raw
path conversion boundary: it rejects empty and NUL-containing inputs, normalizes `.`/`..` and
repeated separators, and enforces path/component limits. Absolute inputs are normalized directly;
relative inputs use the registered working-directory provider, and relative `..` remains at root
when there is no parent. Paths are bytes rather than UTF-8 because the kernel ABI does not require
filenames to be Unicode.

The global VFS interface resolves every relative input through one registered working-directory
provider before dispatching the operation. The process subsystem registers that callback during
initialization and retains ownership of cwd state. Absolute paths bypass the callback. Provider
execution and path normalization complete before any filesystem or mount-table operation begins.

Mount resolution selects the longest matching component boundary. The resolved filesystem
receives a path local to its mount: the unchanged absolute path for the root mount, and
otherwise the mount prefix stripped without a leading separator, so `/dev/fb0` at a `/dev`
mount arrives as `fb0`. The mount point itself arrives as the root path. A mount owns an
`Arc<dyn FileSystem>` and active-handle counter; unmount is rejected while any file or
directory from that mount remains active.

## Global interface and ownership

One `Vfs` is registered globally before process file operations begin. Free functions accept raw
path bytes, resolve them exactly once, and delegate to the global instance. `Vfs` methods, mount
routing, and `FileSystem` callbacks accept only `ResolvedPath`; they never repeat raw validation or
cwd lookup. Filesystems own persistent filesystem state. `VfsFile` owns a boxed `FileHandle`, while
`VfsDirectory` owns an opening-time directory-entry snapshot. Both retain an active inode handle
until close/drop so namespace mutations and unmount cannot invalidate an open object.
These VFS-owned types implement the descriptor-layer `File` and `Directory` capabilities directly;
the descriptor crate remains independent of VFS and contains no concrete file implementations.
`VfsFile::is_terminal` asks the boxed `FileHandle`: regular files keep the default `false`, while a
character-device handle (e.g. a pty slave) reports `true`, so `isatty` works on `devfs` descriptors.

Creation operations receive validated `FilePermissions` from their caller. In particular, directory
creation passes the requested permission bits through the global facade, mount routing, and the
filesystem callback without replacing them with a VFS default. Credential-based filtering and
process umask application remain outside VFS.

Symbolic-link targets remain raw validated bytes because a relative target is stored data whose
meaning depends on the directory containing the link. The link location itself is a
`ResolvedPath`, and `read_link` returns the stored target without resolving it. Normal metadata
queries follow the final symbolic link, while `symlink_metadata` reports the link inode itself.

Filesystem traits are synchronous and return `VfsError`. Filesystem-specific types and errors must
be translated inside adapters such as `roxy-ext4`.

## Invariants and limits

- Mount points are normalized and unique.
- Raw bytes do not cross the global facade except as symbolic-link target data.
- Mount routing and global path handling receive only normalized absolute paths; filesystem
  callbacks receive the mount-local form described above.
- The working-directory provider may acquire the process-table lock, but it must not perform VFS
  operations or retain that lock after returning the cwd snapshot.
- Path resolution never matches a partial component such as `/mnt` against `/mnt2`.
- Filesystem callbacks are not invoked while the mount-table lock is held when that would allow
  re-entry or long I/O.
- Global registration occurs once; use before initialization is an explicit error.

The current VFS has no namespaces, per-process roots, permission credentials, page cache, or
asynchronous I/O. Relative global operations require a currently scheduled process after the
working-directory provider has been registered.
