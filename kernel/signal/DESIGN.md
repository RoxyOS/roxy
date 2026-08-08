# Signal Design

## Purpose and scope

`roxy-signal` owns ABI-neutral signal identities and the actions selected when they are delivered.
It contains no process-table access, scheduler integration, userspace ABI layout, or signal-frame
construction.

## Ownership and extension

`Signal` identifies a supported process-directed signal. `Signal::default_action` explicitly maps
each signal to a `DefaultAction`; the match is intentionally kept local so new signals and future
actions, such as userspace handlers, have one policy definition. The process delivery path retains
the originating signal separately when it needs to encode a terminating wait status.

`roxy-process` owns pending signal queues and executes actions against a target process. The
syscall subsystem remains responsible for translating an ABI-specific signal number into `Signal`
when a userspace sending interface is added.

## Limits

The initial signal set includes the conventional process, fault, timer, child, and terminal signal
identities. `SIGHUP`, `SIGINT`, `SIGKILL`, `SIGPIPE`, `SIGALRM`, `SIGTERM`, `SIGUSR1`, and
`SIGUSR2` terminate; `SIGCHLD` and `SIGWINCH` are ignored. Core-dump, stop, continue, and terminal
job-control actions map to `Unsupported` and must be rejected before they enter a process queue.
Signal masks, user handlers, realtime queues, process groups, and userspace signal ABI records are
not implemented.
