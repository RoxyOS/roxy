# Ext4 Design

## Purpose and scope

`roxy-ext4` adapts the upstream `ext4plus` implementation to the repository's block-device and
VFS traits. It supplies a mounted, readable and writable ext4 filesystem; it does not choose
mount points, own the global VFS registry, or implement path normalization.

## Ownership and adapters

`Ext4FileSystem` owns the ext4plus filesystem object and retains the static block device for its
lifetime. `DeviceIo` translates ext4plus requests into complete `BlockDevice` operations and maps
errors at the adapter boundary. The VFS owns the `Arc<dyn FileSystem>` mount reference after
mounting.

Mutating ext4 operations share an `Arc<Lock<()>>` mutation gate. The gate serializes operations
that ext4plus cannot safely interleave; read-only operations may use the filesystem's own
concurrency guarantees.

## Invariants and flow

Loading must finish successfully before an instance is exposed to VFS. VFS resolves a path and
passes normalized local paths to this filesystem. File handles remain valid until their active
reference is released; unmount is controlled by VFS and is rejected while active handles exist.

File and directory creation use the validated permission bits supplied by VFS when constructing
the ext4 inode. The adapter adds the inode type bits but does not apply a process umask or replace
the requested permissions with filesystem defaults.

Errors from ext4plus are mapped to `VfsError` without leaking upstream types through the public
API. `sync` must reach the underlying filesystem and block device when durability is requested.

## Limits

The adapter currently depends on the selected ext4plus feature set and synchronous block I/O. It
does not provide journaling policy, snapshots, quota management, or a second filesystem format.
