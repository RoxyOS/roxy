# Block Device Design

## Purpose and scope

`roxy-block` defines the synchronous block-device contract consumed by filesystems. It provides
block geometry, aligned reads and writes, flushing, and a RAM-backed implementation for boot and
tests. It does not implement filesystems, caching, partitioning, or path resolution.

## Responsibilities and ownership

`BlockDevice` implementations own or retain their storage and must be `Send + Sync`. Callers own
the destination or source buffers and choose block ranges. The block layer does not retain I/O
buffers after an operation returns.

The `RamDisk` borrows a static, block-aligned source image and stores modified blocks in a locked
copy-on-write map. Reads prefer an overridden block and otherwise use the source image. Writes
replace complete logical blocks without copying untouched image data. This permits a bootloader
module larger than the kernel heap to remain mounted while heap consumption scales with actual
filesystem mutations. `flush` is a successful no-op because both layers are memory resident.

## Invariants

- `block_size()` and `block_count()` describe one stable logical geometry.
- A RAM-disk source remains valid and immutable for the device lifetime.
- Every read and write buffer length is a non-zero multiple of the logical block size.
- A request range must fit entirely within the device; partial I/O is not reported as success.
- Implementations return `BlockError` instead of panicking for caller-controlled alignment and
  range errors.

## Integration flow

Filesystem adapters translate a filesystem byte request into complete block operations, pass those
operations to a `BlockDevice`, and map device failures into their own error model. A device may
serialize internally, but the block trait itself does not impose scheduling or transaction
semantics.

## Limits and extensions

The current interface is synchronous and has no request queue, discard operation, or asynchronous
completion. New operations should be added only when a filesystem or device needs them; callers
must not infer durability from `write_blocks` without calling `flush`.
