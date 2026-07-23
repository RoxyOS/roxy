# Input Device Design

## Purpose and scope

`roxy-input` defines the shared raw-input boundary used by user-facing TTY adapters. It owns no
hardware, queues, terminal semantics, file descriptors, or output devices. Drivers implement its
single `InputDevice` contract; consumers receive decoded or raw bytes according to the driver's
documented policy.

## Contract and limits

`InputDevice::read_byte` returns the oldest available byte or `None` without blocking. Drivers own
only input production and queue synchronization; TTY adapters own buffer filling and waiting policy.
The current interface deliberately has no echo, canonical processing, terminal attributes, signals,
blocking API, or device enumeration. Those are future TTY or line-discipline responsibilities.
