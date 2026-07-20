# Syscall Design

## Purpose and scope

`roxy-syscall` is the userspace ABI boundary. It owns syscall numbering, registration, dispatch,
raw-argument parsing, errno conversion, and mandatory unsupported-operation reporting. Actual
process, memory, file, futex, and time operations remain in their owning subsystems.

## Registry and dispatch

The static syscall table is validated for duplicate numbers before the architecture entry is
configured. The architecture backend supplies a normalized `RawSyscall`. Most handlers receive six
raw argument words; handlers such as fork may request the saved user context explicitly.

Handlers follow three stages: parse raw values into typed data, validate all userspace-controlled
state, then call the owning subsystem. The implementation stage should remain a small delegation,
not a second copy of subsystem policy.

## Userspace memory contract

Pointers are interpreted only through typed `UserAddress` values and the current process's
`AddrSpaceHandle`. A handler must copy any path, array, or structure that must survive an address
space change before invoking that change. Cross-page reads are valid when every covered page is
mapped and accessible.

`execve` therefore parses path, argv, and envp completely before process image replacement. On
success it invokes the architecture's fresh-user resume path and never returns to the old image.

## Errors and unsupported behavior

Subsystem errors are translated to stable ABI errno values at this boundary. Invalid userspace
addresses return `EFAULT`; size limits and format failures use their defined errno values. Missing
kernel functionality must emit the centralized unconditional `UNSUPPORTED` diagnostic before an
error is returned, including operation, argument, PID/TID, and errno.

## Limits

The ABI is currently Roxy-specific and manually mirrored by the Roxy mlibc sysdeps. Every ABI
change must keep syscall numbers, Rust/C declarations, registry tests, and userspace symbols in
sync.
