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
identities, plus the realtime signals. Stop and continue are supported: a stop signal suspends the
process until `SIGCONT` resumes it, and the resumption stays visible to a parent waiting with
`WCONTINUED`. There is no core-dump action. Process groups and their signalling live in
`roxy-process`, and userspace signal ABI records live in the syscall subsystem; this crate holds
only signal identity, default-action policy, and the mask type.
