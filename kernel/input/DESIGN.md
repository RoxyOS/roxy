# Input Device Design

## Purpose and scope

`roxy-input` defines the shared raw-input boundary used by user-facing TTY adapters. It owns no
hardware, queues, terminal semantics, file descriptors, or output devices. Drivers implement its
single `InputDevice` contract; consumers receive raw physical key events.

## Contract and limits

`InputDevice::read_key` returns the oldest available `KeyEvent` or `None` without blocking.
`KeyEvent` carries a layout-neutral `KeyCode` (the single source of truth for key identity) and a
pressed/released `KeyState`. Drivers own only input production and queue synchronization; layout
mapping, byte encoding, buffer filling, and waiting policy belong to consumers. A consumer may
register an `InputListener`; the driver calls it after queueing input so the consumer can wake its
own readiness waiters. The notification reports only that input may be available and does not
consume, encode, or interpret the event. `InputListeners` owns the weak listener collection,
registration, expired-listener cleanup, and notification traversal so hardware drivers only
publish the event transition.

Layout is deliberately absent from this boundary: `KeyCode` names physical keys, and the mapping
to characters, escape sequences, or ABI-specific key codes is the responsibility of each consumer
(the TTY through `pc_keyboard`, a graphics stack through its own layout engine). The current
interface has no echo, canonical processing, terminal attributes, signals, blocking API, or
device enumeration. TTY adapters and line disciplines own those policies above this raw-input
boundary.
