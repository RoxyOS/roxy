# Input Device Design

## Purpose and scope

`roxy-input` defines the shared raw-input boundary used by user-facing TTY adapters. It owns no
hardware, queues, terminal semantics, file descriptors, or output devices. Drivers implement its
single `InputDevice` contract; consumers receive decoded or raw bytes according to the driver's
documented policy.

## Contract and limits

`InputDevice::read_event` returns the oldest available `InputEvent` or `None` without blocking.
`Character` carries Unicode text and control characters; `Key` carries non-character keys with
pressed/released state. Drivers own only input production and queue synchronization; TTY adapters
own byte encoding, buffer filling, and waiting policy. A consumer may register an `InputListener`;
the driver calls it after queueing input so the consumer can wake its own readiness waiters. The
notification reports only that input may be available and does not consume, encode, or interpret
the event. `InputListeners` owns the weak listener collection, registration, expired-listener
cleanup, and notification traversal so hardware drivers only publish the event transition.
The current interface deliberately has no echo, canonical processing, terminal attributes,
signals, blocking API, or device enumeration. TTY adapters and line disciplines own those policies
above this raw-input boundary.
