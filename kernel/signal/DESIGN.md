# Signal Design

## Purpose and scope

`roxy-signal` owns ABI-neutral signal identities and the actions selected when they are delivered.
It contains no process-table access, scheduler integration, userspace ABI layout, or signal-frame
construction.

## Ownership and extension

`Signal` identifies a supported process-directed signal. `Signal::default_action` explicitly maps
each signal to a `SignalAction`; the match is intentionally kept local so new signals and future
actions, such as userspace handlers, have one policy definition. The process delivery path retains
the originating signal separately when it needs to encode a terminating wait status.

`roxy-process` owns pending signal queues and executes actions against a target process. The
syscall subsystem remains responsible for translating an ABI-specific signal number into `Signal`
when a userspace sending interface is added.

## Limits

The initial signal set contains only `SIGINT`, `SIGKILL`, and `SIGTERM`; every one terminates the
target. Signal masks, user handlers, stop and continue actions, realtime queues, process groups,
and userspace signal ABI records are not implemented.
