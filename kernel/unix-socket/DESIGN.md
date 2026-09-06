# Unix Socket Design

> **IMPORTANT**: The user does not review this subsystem's code. Every change is its own last
> careful review, so the following are mandatory, not aspirational:
>
> - **Maintainability and extensibility are hard requirements.** Keep the object model, the
>   lifecycle state machine, and the ownership boundaries between `Socket`, `Connected`, and
>   `BoundSocket` explicit and honest. Prefer small, verifiable steps over cleverness, and update
>   this document in the same change so it never drifts from the implementation.
> - **Test coverage must be as broad as practical.** Every behavior change adds or extends kernel
>   tests covering the success path, every error path, state-machine rejections, registry
>   lifecycle, and blocking or readiness semantics where testable. A change without tests is an
>   incomplete change.
> - **Validation gates stay green.** Existing kernel tests and workspace checks must pass before
>   a change is considered done; a red gate is treated as a defect of the change, not a known
>   baseline.

## Purpose and scope

`roxy-unix-socket` owns local, bidirectional Unix stream sockets in all lifecycle states:
anonymous paired sockets, addressed (filesystem-named) sockets, connection establishment, and
data transfer. `stream::pair` and `stream::socket` are the public creation entry points; both
return FD-layer open files. Datagrams, abstract-namespace addresses, descriptor flags such as
`SOCK_CLOEXEC`, nonblocking mode, ancillary data, and socket-specific control operations are
outside the current scope.

## Object model

POSIX exposes one polymorphic socket fd that can play both the client and the server role. That
role polymorphism is confined to the fd-visible `Socket` type; the real behavior lives in two
peer objects with independent lifecycles.

- `Socket` is the only FD-layer `File` type. It owns the lifecycle state machine
  (`Initial`, `Bound`, `Connected`), the `SocketOps` implementation, state-transition errors, and
  unbinding on drop. It never transfers data itself.
- `Connected` is the payload of the connected state: one side of an established connection. It
  owns buffered transfer, blocking reads and writes, readiness, and half-close semantics for its
  side. `stream::pair` returns two sockets already in this state, which is why `socketpair`
  behavior is unchanged by addressing support.
- `BoundSocket` is the bound server-side object held by the `Bound` state. It owns the accept
  backlog, blocking `accept`, and connection establishment. The bound socket itself never becomes
  connected: a server such as an X server stays bound for its entire lifetime, and every accepted
  connection is a fresh connected `Socket`.

## Address registry

The bound-socket registry maps normalized addresses to live `BoundSocket` objects. The registry
holds weak references and prunes dead entries on lookup and insert, so an address disappears when
the last bound socket closes; the owning socket also removes its entry explicitly on drop. Each
address is held by at most one live socket, and a filesystem entry at the same path is rejected
by the bind caller before the registry is consulted.

Connection establishment is immediate and non-blocking: `connect` locates the listener, checks
the backlog capacity, creates one connection whose client side becomes the caller's `Connected`
payload and whose server side is queued for `accept`. A missing or dead listener, a
bound-but-not-listening socket, and a full backlog all report connection refusal. Accepted
connections are named in the order they arrive; there is no priority or fairness policy.

## Ownership and transfer state

The connection owns two receive states under one lock. A write through one side appends to the
peer's receive state, while a read drains the caller's receive state. Each direction has an
independent 64 KiB capacity.

A connected side is closed when its enclosing `OpenFile` is finally dropped. This naturally
preserves the connection across forked descriptor-table references. Closing marks that side
unavailable, discards bytes that can no longer be received, and wakes the peer. The peer drains
already-buffered bytes before observing EOF; a subsequent write fails as a broken pipe.

## Blocking and readiness

An empty read, a write to a full peer buffer, and an `accept` on an empty backlog block through
the scheduler. Each operation checks state, registers its keyed poll listener, and prepares the
block while holding the relevant state lock. It releases the lock before switching, so a
state-changing peer cannot notify between the readiness check and block preparation. State
mutations release their lock before notifying listeners to avoid nesting state and scheduler
locks.

Read readiness means buffered data or peer closure. Write readiness means an open peer with
buffer capacity. A bound socket reports readable while its backlog holds pending connections.
Peer closure also reports hangup. The implementation blocks through the kernel scheduler and does
not expose a nonblocking mode.
