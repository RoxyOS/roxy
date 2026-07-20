# Kernel Test Design

## Purpose and scope

`roxy-test` provides the distributed kernel unit-test registry and the `kernel_test!` registration
macro. It allows subsystem tests to be linked into the kernel-test image without creating a
dependency from the test runner back into every subsystem.

## Registration and ownership

Each macro invocation defines one function and inserts a `TestCase` into the link-time distributed
slice. The core test harness owns iteration, reporting, and QEMU exit behavior. This crate owns only
registration metadata and the macro contract.

Test names must be globally meaningful and stable enough to identify a failure. Test functions
must not depend on source order because link-time slice ordering is not an execution contract.

## Execution model

Tests run inside the initialized kernel environment selected by the `kernel-test` feature. They may
exercise architecture and allocator behavior unavailable to host tests, but must restore global
state or use one-time fixtures compatible with the harness lifecycle.

## Limits

The repository currently has this distributed kernel harness and ordinary host Cargo tests where
available; it has no separate CI layer defined by this subsystem. The macro must not contain
test-name-specific behavior or silently skip unsupported kernel functionality.
