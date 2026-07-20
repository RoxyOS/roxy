# Architecture Design

## Purpose and scope

`roxy-arch` is the architecture boundary for kernel entry, exceptions, local interrupts, CPU
identity, user contexts, and privileged user transitions. Architecture-independent crates depend
on its contracts instead of accessing registers, descriptor tables, or assembly directly.

The current backend is x86_64. Adding another architecture means implementing the sealed
`Architecture` contract and its context types; it does not change process, syscall, or scheduler
ownership rules.

## Responsibilities and ownership

- The selected backend owns architecture entry code, descriptor tables, syscall instructions, and
  architecture-specific saved-context layout.
- `RawSyscall` is the normalized boundary value passed from the backend to the syscall subsystem.
- `UserContext` describes the user resume state needed by fork and ordinary syscall return.
- The architecture layer does not own process address spaces, file descriptors, or syscall policy.

## Invariants and flows

Initialization installs exception and local-interrupt callbacks before interrupts are enabled.
Syscall entry normalizes the raw register frame, dispatches one handler, and restores the saved
userspace return context when that handler returns. A fresh `execve` image uses the separate
`resume_user` contract so it can enter new RIP/RSP values without returning through the old
userspace frame.

Architecture methods that enter userspace require the active page table to map the supplied user
addresses. Unsafe backend code must keep this obligation local and document it at the call site.

## Failure and limits

Invalid CPU state, unsupported exception forms, non-canonical user pointers, and repeated
initialization are kernel faults rather than recoverable userspace errors. The backend currently
assumes the BSP-oriented CPU model exposed by the rest of the kernel and provides no portability
promise beyond the implemented target.

## Design decisions

The trait exposes semantic operations such as `resume_user` and `set_kernel_stack_top` rather than
register-level helpers. This keeps syscall and process code independent of x86 register names and
prevents architecture details from leaking into generic subsystems.
