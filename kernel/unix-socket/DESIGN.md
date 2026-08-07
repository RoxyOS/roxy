# Unix Socket Design

## Purpose and scope

`roxy-unix-socket` owns local, bidirectional Unix stream connections. Its current public surface is
`stream::pair`, which creates two already-connected open files. Addressed sockets, connection
establishment, datagrams, descriptor flags, ancillary data, and socket-specific control operations
are outside the current scope.

## Ownership and state

Each endpoint is an FD-layer `File` object and holds one side identifier plus an `Arc` to the shared
connection. The connection owns two receive states under one lock. A write through one endpoint
appends to the peer's receive state, while a read drains the caller's receive state. Each direction
has an independent 64 KiB capacity.

The endpoint is closed when its enclosing `OpenFile` is finally dropped. This naturally preserves
the connection across forked descriptor-table references. Closing marks that side unavailable,
discards bytes that can no longer be received, and wakes the peer. The peer drains already-buffered
bytes before observing EOF; a subsequent write fails as a broken pipe.

## Blocking and readiness

An empty read and a write to a full peer buffer block through the scheduler. The operation checks
state, registers its keyed poll listener, and prepares the block while holding the connection lock.
It releases the lock before switching, so a state-changing peer cannot notify between the readiness
check and block preparation. State mutations release the connection lock before notifying listeners
to avoid nesting the connection and scheduler locks.

Read readiness means buffered data or peer closure. Write readiness means an open peer with buffer
capacity. Peer closure also reports hangup. The implementation assumes the current BSP-oriented
kernel execution model and does not expose a nonblocking mode.
