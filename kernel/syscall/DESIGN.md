# Syscall Design

## Purpose and scope

`roxy-syscall` is the sole userspace ABI boundary. It owns ABI personality selection, syscall
numbering, registration, dispatch, raw-argument parsing, record layout, errno conversion, and
mandatory unsupported-operation reporting. Actual process, memory, file, futex, and time
operations remain in their owning subsystems.

## ABI isolation

The long-term architecture permits multiple Unix-like ABI personalities, including Linux-, BSD-,
and Solaris-compatible interfaces, while the current implementation exposes only the Roxy ABI.
Every personality-specific record layout, `#[repr(C)]` type, padding field, size or offset
assertion, request number, and raw userspace pointer interpretation is private to this subsystem.

Handlers decode those representations into ABI-neutral kernel types before calling another
subsystem and encode returned domain values only at the userspace copy boundary. Process, FD, TTY,
filesystem, scheduler, and other domain APIs must never accept or return a personality-specific
record. Adding an ABI personality therefore adds adapters here rather than conditional layouts or
compatibility branches throughout the kernel.

The ioctl family follows the same rule: the request numbers, such as the framebuffer `GET_INFO`,
`TAKE_CONTROL`, and `RELEASE_CONTROL`, and the `roxy_framebuffer_info` record are private to this
subsystem, whose adapter decodes them into the fd layer's layout-neutral `FbInfo` before dispatch
and encodes it back at the userspace copy boundary. Size and offset assertions pin the checked
`x86_64` layout. `TAKE_CONTROL` and `RELEASE_CONTROL` carry no argument; the frame is claimed for
the calling process, which the framebuffer device resolves, not this layer.

`ioctl/numbers.rs` owns the request numbers. They form a flat, global space in which every device
family owns a `0x100`-aligned block, so two families cannot collide as either grows, and no number
carries a direction or size encoding: dispatch matches whole numbers, and each handler already
knows the direction and record layout of its own request. Adding a family means claiming a new
block in that module; its test checks that no request has left the block that owns it.

The space is also one narrow window, `[SPACE_BASE, SPACE_END)`, placed above every number another
personality produces for these requests, so a request outside it is never mistaken for one of
ours. `in_space` makes that distinction available to dispatch, which reports a request outside the
window as foreign and a request inside it that no handler defines as an undefined request of our
own; both keep `ENOTTY`. Assertions beside the constants pin the base above Linux's `_IO(type, nr)`
range and the end below its `_IOC` direction bit and below `2^31`, so every request of ours fits a
signed `int`.

## Registry and dispatch

`numbers.rs` owns the syscall numbers. They are `SYSCALL_BASE` plus each syscall's index, so the
whole space sits above every number another personality uses for the syscalls this kernel provides,
and the low range stays free for a Linux-compatible personality to serve unshifted. Dispatch
separates the two cases a number it cannot resolve can be: one below the base is a foreign request,
which is what a program with its own syscall layer sends, and one inside the space is an undefined
syscall of our own. Both are reported through the centralized unsupported path and return `ENOSYS`.

The static syscall table is validated for duplicate numbers before the architecture entry is
configured. The architecture backend supplies a normalized `RawSyscall`. Most handlers receive six
raw argument words; handlers such as fork may request the saved user context explicitly.

Ordinary handlers declare their ordered arguments through the syscall registration macro. The
generated adapter converts each raw word into the declared primitive or boundary type and then
calls the typed handler. Every fallible conversion declares its ABI error at the registration site,
such as `Fd => BadFd` or `UserAddress => Fault`; the parser applies that error without introducing
policy-specific wrapper types. Validation whose ordering affects observable behavior remains
explicit in the handler; for example, `poll` accepts an arbitrary pointer with a zero count, and
`ioctl` resolves the descriptor before validating its pointer argument. Context-sensitive handlers
such as `fork` continue to receive the complete saved syscall context.

Handlers follow three stages: parse raw values into typed data, validate all userspace-controlled
state, then call the owning subsystem. The implementation stage should remain a small delegation,
not a second copy of subsystem policy.

`read` uses a bounded kernel transfer buffer and performs exactly one underlying file read per
syscall. It therefore returns at most 4096 bytes even when userspace requests more; callers that
require additional data must issue another read. This uniform short-read policy keeps file-type
semantics, including terminal line boundaries, inside the owning file implementation.

`writev` (syscall 71) gathers data from a userspace `iovec` array and writes it through the
addressed descriptor, honoring the file's own nonblocking flag. The `struct iovec` record and the
gather/write helper live in the shared `syscalls::iovec` module, which both this syscall and the
`recvmsg`/`sendmsg` handlers reuse, so the record layout and its 16-byte size assertion exist in
only one place. The handler caps the iovec count at Linux's `IOV_MAX` (1024) and returns `EINVAL`
above it; a `BrokenPipe` write delivers `SIGPIPE` and returns `EPIPE`, matching `write`. Like
`write`, a short write on one iovec stops the loop and reports the bytes written so far.

Path-based `stat` and `open` copy the userspace byte string before passing it to the global VFS
interface. The VFS leaves absolute paths independent of cwd and obtains the process-owned cwd
through its registered provider only for relative paths. Syscall handlers do not duplicate path
normalization or process-state lookup. `open` records `O_CLOEXEC` as a close-on-exec descriptor
flag, which `execve` honors by closing those descriptors when replacing the image. Path-based
`stat` accepts `AT_SYMLINK_NOFOLLOW` and uses the VFS link-metadata operation to report the final
symbolic link instead of its target.

Filesystem mutation syscalls use the shared `Path` argument type, which copies the userspace
string and rejects an empty path during argument parsing. The `mkdirat`, `unlinkat`, `readlinkat`,
`linkat`, `symlinkat`, and `renameat` handlers currently accept only `AT_FDCWD`; descriptor-relative
resolution remains unsupported and is reported through the centralized diagnostic path. `sync`
delegates to the global VFS, while `fsync` resolves an open file and dispatches synchronization
through the FD object boundary. `ftruncate` accepts a descriptor and a nonnegative Roxy `off_t`
length, dispatches the length change through that boundary, and leaves the shared offset unchanged.

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

`sigprocmask` decodes the Roxy `sigset_t` ABI and atomically blocks, unblocks, or replaces the
current process's signal mask. The record is the 128-byte set every libc uses, but only its first
word carries signals: the remaining words are reserved and ignored, because `sigfillset` fills the
whole record and Linux never sees those bytes at all. A null input set queries without changing the
mask, and a non-null
old-set output receives the mask active before the operation. The output range is validated before
state changes. The process subsystem removes `SIGKILL` and `SIGSTOP` from every installed mask.
`sigaction` supports querying and installing `SIG_DFL`, `SIG_IGN`, and user-handler dispositions
through the Roxy x86_64 ABI record. Null action and old-action pointers independently select query
and output behavior. The syscall validates its input and output before changing process state.
A handler disposition records the user function address, the per-handler mask, and whether it was
installed with `SA_SIGINFO`; the ABI
restorer field is ignored because the kernel injects its own `sigreturn` trampoline into every
process image. `SA_SIGINFO` switches the handler to the three-argument form and is the only flag
accepted; all other flags use the centralized diagnostic path.
`SIGKILL` and `SIGSTOP` cannot be ignored.

`sigreturn` (syscall 54) is a registry handler with a dedicated `Handler::Exit` variant whose
function returns `SyscallExit` directly, replacing the syscall-return contract itself. It asks
`roxy-process` to pop and validate the most recent signal frame against the caller's stack
pointer, and returns a full context restoration. Because its handler returns `SyscallExit`, it
skips the signal-delivery step ordinary value-returning syscalls apply on exit. A spurious call
returns `EINVAL`. The `syscall!` macro exposes a `-> SyscallExit` form for handlers that own
their resume contract.

`send_signal` is the Roxy ABI operation backing mlibc's `kill`. It accepts a positive process ID
and a Linux-compatible signal number, translates both into ABI-neutral process and signal types,
and delegates queuing to `roxy-process`. It supports only direct-process targets: zero, negative,
and signal-zero selectors are rejected with an `UNSUPPORTED` diagnostic. A missing process returns
`ESRCH`; a signal whose default action is not yet implemented returns the diagnostic `ENOTSUP`
path. The syscall layer alone translates signal numbers; `roxy-process` never depends on a
personality's numeric signal ABI.

`open_dir` creates a descriptor backed by an opening-time VFS directory snapshot. `read_entries`
serializes that descriptor into fixed-size Roxy x86_64 `dirent` records and advances the shared
open-file position by entry count. A writable userspace range is validated before the position is
advanced; EOF returns zero bytes, and `seek` to entry zero implements `rewinddir`.

`chdir` resolves its path against the old cwd, verifies through VFS metadata that the result is a
directory, and only then replaces the process-owned normalized absolute cwd. Failed validation
leaves the existing cwd unchanged.

`getcwd` snapshots the process-owned normalized absolute cwd, appends a null terminator, and copies
the complete result to a caller-provided writable buffer. Success returns the copied byte count,
including the terminator. A buffer shorter than the complete result returns `ERANGE`; an invalid or
non-writable userspace range returns `EFAULT`. No process-table lock spans result encoding or the
userspace write.

`uname` writes a fully initialized Roxy x86_64 `utsname` record with static system identity. The
syscall layer owns its six 65-byte, null-terminated ABI fields; it does not expose the record or
identity strings to ABI-neutral kernel subsystems. Hostname configuration and runtime kernel build
metadata are not yet supported.

`poll` decodes the userspace `pollfd` array inside this subsystem and queries each descriptor's
ABI-neutral readiness through `roxy-fd`. For a nonzero timeout it rechecks in a loop: with
interrupts disabled, it registers one `roxy-poll` listener with each source before re-checking
readiness, adds a cancelable monotonic timer registration when finite, and blocks against the
listener's wake latch when still unready. When the latch shows an owed wake the block is skipped
and the thread keeps running to re-query readiness. The register-first order plus the latch close
the SMP lost-wakeup windows; a notification or deadline wake always causes a fresh readiness query
before results are encoded. It reports TTY and regular-file readiness and returns `POLLNVAL` for invalid
descriptors. No-descriptor finite polls are sleeps; an infinite no-descriptor poll remains blocked.
Signals and temporary signal-mask replacement remain unsupported.

`ppoll` shares `poll`'s descriptor readiness and timer-wait implementation, but decodes its
relative timeout from the Roxy mlibc `timespec` ABI at nanosecond precision. A null timeout waits
indefinitely. A non-null signal mask temporarily replaces the current process mask for the
duration of the wait, then restores the old mask before returning to userspace. An unmasked pending
signal wakes the waiting thread, returns `EINTR`, and is processed before that restoration.

`pselect` adapts the Roxy mlibc 1024-bit `fd_set` ABI to the same poll readiness and timer-wait
path. It combines requested read, write, and exceptional events for each descriptor, then replaces
each non-null input set with its ready descriptors. A non-null signal mask has the same temporary
replacement and `EINTR` behavior as `ppoll`. Roxy mlibc's standard `select` wrapper uses this
`pselect` sysdep after translating its timeout to `timespec`.

`sleep` copies a Roxy x86_64 `timespec` request and validates nonnegative seconds with
nanoseconds in the half-open range `[0, 1_000_000_000)`. It converts the relative duration into a
monotonic deadline and delegates blocking to the timer-wait subsystem. Signals are not implemented, so a
sleep cannot be interrupted and no remaining duration is reported.

`ioctl` validates and resolves the descriptor before decoding the raw request number. For terminal
requests, the syscall layer copies the Roxy mlibc `termios` or `winsize` record between userspace
and an initialized typed kernel value. Setters own
their copied value; getters borrow a typed local that the file object fills before it is copied
back. The FD layer owns locked object dispatch, while the syscall layer maps operation errors to
errno. Unknown requests return `ENOTTY` without a diagnostic. Consequently an invalid descriptor
returns `EBADF` even when the request is unknown. A file object's `IoctlError::NotTty` also maps to
`ENOTTY`; rejected unsupported terminal fields use the centralized diagnostic and `ENOTSUP` path.

The current process model has no stored credentials and treats every process as the root identity.
`getuid`, `geteuid`, `getgid`, and `getegid` therefore return real and effective user and group IDs
of `0` without consulting process state. Credential storage, mutation, and permission enforcement
remain outside the supported ABI.

## Userspace memory contract

Pointers are interpreted only through typed `UserAddress` values and the current process's
`AddrSpaceHandle`. A handler must copy any path, array, or structure that must survive an address
space change before invoking that change. Cross-page reads are valid when every covered page is
mapped and accessible.

Ordinary null-terminated byte strings parse into the syscall-owned `CString`, which owns a
`Vec<u8>` and dereferences to that vector without imposing UTF-8 validity. Parsing reads from the
current address space across page boundaries up to the VFS path-length limit. `execve` uses the
same type for its path and lets the process subsystem enforce the final user-stack size.

User arrays with an explicit address and element count use `Slice<T>`. The wrapper is constructed
at the syscall's required validation stage rather than consuming raw arguments automatically; its
unsafe `read`, `read_with_limit`, and `write` methods copy through the current address space and
enforce allocation, capacity, address-range, and byte-size bounds. Offset slices support bounded
chunked transfers without changing the original userspace range. Null-terminated pointer arrays
such as `execve`'s `argv` and `envp` instead use `CStringArray`, because their ABI provides a
terminator rather than an element count.

Fixed-layout input records implement `SyscallArg` beside their owning syscall and copy themselves
from userspace during argument parsing. Fixed-layout outputs use `Out<T>`, which validates and
retains only the destination address so parsing never reads an output buffer. Request-dependent
interfaces such as `ioctl` select the concrete input or output type only after decoding the request.
Shared unsafe copy primitives form byte slices for single records and record arrays, but every call
site retains the local proof that its checked layout has no implicit padding, initializes every
output byte, and accepts arbitrary input bytes. The copy primitives do not define layouts, validate
request-specific lengths, or select transfer direction.

`execve` therefore parses path, argv, and envp completely before process image replacement. On
success it invokes the architecture's fresh-user resume path and never returns to the old image.

## Errors and unsupported behavior

Subsystem errors are translated to stable ABI errno values at this boundary. Invalid userspace
addresses return `EFAULT`; size limits and format failures use their defined errno values. Missing
kernel functionality must emit the centralized unconditional `UNSUPPORTED` diagnostic before an
error is returned, including operation, argument, PID/TID, and errno. The `ioctl` parser reports a
request no handler serves the same way, separating a foreign request from an undefined one of its
own, and returns `ENOTTY`: the errno a probe expects from a device that does not serve the
request.

## Limits

The only active personality is currently Roxy-specific and manually mirrored by the Roxy mlibc
sysdeps. Every ABI change must keep syscall numbers, private layout adapters, registry tests, and
userspace symbols in sync. The ioctl request numbers are Roxy's own rather than Linux's, allocated
in `ioctl/numbers.rs` and mirrored by the Roxy mlibc `abi-bits/ioctls.h`; the request *names* keep
their conventional Unix spelling so ported userspace compiles unchanged, while the values stay
disjoint from every other personality's. The `termios` and `winsize` records copied across this
boundary keep mlibc's layout. Requests the kernel does not implement, such as `TIOCGSID`,
`TIOCMGET`, and the `SIOC*` socket requests, keep their generic mlibc definitions and return
`ENOTTY`; other ioctl families remain unsupported.

The `fcntl` commands form a single family with one owned space, mirrored by the Roxy mlibc
`abi-bits/fcntl.h`. A supported command is `COMMAND_BASE` plus its index, and every command the
kernel does not implement is pinned to one `UNSUPPORTED_COMMAND` value below that base. The base
sits above every command number another personality uses, so a command below it can only be
foreign, and the handler reports the header's unsupported marker and another libc's numbering as
distinct diagnostics. All unsupported commands sharing one value is deliberate: telling them apart
would preserve information no caller can act on.

`socketpair` is syscall 48 and accepts only `AF_UNIX`, `SOCK_STREAM`, and protocol zero. It asks
`roxy-unix-socket` to create the connected files, then owns descriptor insertion and the checked
copy of the descriptor pair to userspace.

The addressed socket family adds syscalls 49-53: `socket`, `bind`, `listen`, `accept`, and
`connect`. `socket` accepts only `AF_UNIX`, `SOCK_STREAM`, and protocol zero; `SOCK_CLOEXEC` and
`SOCK_NONBLOCK` and all other types, domains, and protocols emit the centralized unsupported
diagnostic. Callers that tolerate rejection, such as libxcb's `SOCK_CLOEXEC` fallback, receive
`EINVAL`. `bind` and `connect` decode the personality-private `sockaddr_un` record in this
subsystem, accepting either a length-bounded or NUL-terminated path, rejecting abstract
addresses, and normalizing the path through the VFS boundary before handing raw bytes to the
socket object. `bind` additionally refuses addresses occupied by a filesystem entry.
`accept` inserts the returned connection as a fresh descriptor. Socket-specific failures map to
`ENOTSOCK`, `EADDRINUSE`, `EISCONN`, `ENOTCONN`, and `ECONNREFUSED`; reads and writes through
unconnected sockets report `ENOTCONN`. The socket subsystem owns buffering, blocking, readiness,
and close state. Peer addresses are not reported: mlibc fills the exact unnamed `AF_UNIX` peer
address that the kernel's refusal of client-side `bind` guarantees.

Other families, types, protocols, and socket operations emit the centralized unsupported
diagnostic when they reach this ABI boundary.

## Clocks

`clock_get` (syscall 8) and `clock_getres` (syscall 83) share one `roxy_clock_result` record and one
clock set, so both the record layout and the identifiers live in the `syscalls::clock` module and
neither handler carries its own copy. Both accept `CLOCK_REALTIME` and `CLOCK_MONOTONIC`;
`clock_get` reports the reading, `clock_getres` the interval in which that clock advances, which
`roxy-time` derives from the periodic timer tick. Every other `clockid_t` returns `EINVAL` through
the centralized diagnostic, so a caller such as Xorg's `GetTimeInMillis` that probes a coarse clock
first falls back to `CLOCK_MONOTONIC` instead of failing.

### `TTYNAME`

`ROXY_SYS_TTYNAME(fd, buf, size)` (72) writes the NUL-terminated openable pathname of the terminal
backing `fd` into a user buffer. The name is owned by the terminal object — the descriptor layer's
ABI-neutral `File::terminal_path` — not synthesized here, so each terminal reports its own path
(`/dev/tty0` for the console, `/dev/pts/N` for a pty slave). Errors: `ENOTTY` when `fd` is not a
terminal, `ERANGE` when the buffer cannot hold the name plus its terminator, `EBADF` for an invalid
descriptor. No structured ABI record is involved; the payload is a plain null-terminated byte
string like `getcwd`.
