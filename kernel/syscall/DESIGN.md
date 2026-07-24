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

`read` uses a bounded kernel transfer buffer and performs exactly one underlying file read per
syscall. It therefore returns at most 4096 bytes even when userspace requests more; callers that
require additional data must issue another read. This uniform short-read policy keeps file-type
semantics, including terminal line boundaries, inside the owning file implementation.

Path-based `stat` and `open` copy the userspace byte string before passing it to the global VFS
interface. The VFS leaves absolute paths independent of cwd and obtains the process-owned cwd
through its registered provider only for relative paths. Syscall handlers do not duplicate path
normalization or process-state lookup. Path-based `stat` accepts `AT_SYMLINK_NOFOLLOW` and uses the
VFS link-metadata operation to report the final symbolic link instead of its target.

Process-identity queries delegate to the process subsystem. `getpid` returns the stable process ID
owned by the current thread's process and does not expose scheduler thread IDs through the ABI.
`getppid` returns the recorded fork parent while that process remains in the process table and
returns `0` for directly spawned or orphaned processes.

`waitpid` accepts a direct child PID or `-1` for any child, with optional `WNOHANG`. It validates a
non-null status output before entering the process wait so `EFAULT` never consumes a zombie. Normal
exit codes use the Linux wait-status layout expected by mlibc. A successful wait returns the reaped
PID, a pending nonblocking wait returns zero, and absence of a matching child returns `ECHILD`.
Process-group selectors, stopped or continued states, and resource usage remain unsupported and
must use the centralized diagnostic path.

`sigprocmask` and `sigaction` have stable syscall numbers for the mlibc ABI, but signal state and
delivery are not implemented. Their handlers do not dereference signal-structure pointers; each
emits the centralized `UNSUPPORTED` diagnostic with the requested mask operation or signal number
and returns `ENOSYS`.

`open_dir` creates a descriptor backed by an opening-time VFS directory snapshot. `read_entries`
serializes that descriptor into fixed-size Roxy x86_64 `dirent` records and advances the shared
open-file position by entry count. A writable userspace range is validated before the position is
advanced; EOF returns zero bytes, and `seek` to entry zero implements `rewinddir`.

`chdir` resolves its path against the old cwd, verifies through VFS metadata that the result is a
directory, and only then replaces the process-owned normalized absolute cwd. Failed validation
leaves the existing cwd unchanged.

The current process model has no stored credentials and treats every process as the root identity.
`getuid`, `geteuid`, `getgid`, and `getegid` therefore return real and effective user and group IDs
of `0` without consulting process state. Credential storage, mutation, and permission enforcement
remain outside the supported ABI.

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
