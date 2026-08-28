# Signal Design

## Purpose and scope

`roxy-signal` owns ABI-neutral signal identities, per-signal default-action policy, and the
domain-wide `SignalSet` mask type. It contains no process-table access, scheduler integration,
userspace ABI layout, or signal-frame construction.

## Ownership and extension

`Signal` identifies a supported process-directed signal. `Signal::default_action` explicitly maps
each signal to a `DefaultAction`; the match is intentionally kept local so new signals have one
policy definition.

`SignalSet` is the single mask representation shared by `roxy-process` state, the syscall layer's
decoded ABI sets, and signal delivery. It is a 64-bit set covering every supported signal;
extended ABI masks (wider sets from future ABI personalities) are rejected at the syscall
boundary before they reach this type.

`roxy-process` owns pending signal queues, per-process dispositions, signal frames, and executes
actions against a target process. The syscall subsystem remains responsible for translating
ABI-specific signal numbers and mask records into `Signal` and `SignalSet`.

## Limits

The initial signal set includes the conventional process, fault, timer, child, and terminal signal
identities. Core-dump, stop, continue, and terminal job-control default actions map to
`Unsupported` and are rejected when sent. Realtime signals, process groups, and userspace signal
ABI records are not implemented. Per-process dispositions remain process-owned rather than part
of this crate's signal policy.
