# Architecture Design

## Purpose and scope

`roxy-arch` is the architecture boundary for kernel entry, exception and interrupt entry
stubs, CPU identity, user contexts, and privileged user transitions. Architecture-independent
crates depend on its contracts instead of accessing registers, descriptor tables, or assembly
directly.

The current backend is x86_64. Adding another architecture means implementing the sealed
`Architecture` contract and its context types; it does not change process, syscall, or scheduler
ownership rules.

## Responsibilities and ownership

- The selected backend owns architecture entry code, descriptor tables, interrupt vector numbers,
  syscall instructions, and architecture-specific saved-context layout.
- `Interrupt` distinguishes local-controller events from ISA IRQ lines while keeping one callback
  contract for IDT entry. x86_64 currently installs IRQ0..IRQ15 at vectors `0x20..0x2f`.
- `RawSyscall` is the normalized boundary value passed from the backend to the syscall subsystem.
- `UserContext` describes the user resume state needed by fork and ordinary syscall return.
- The architecture layer does not own process address spaces, file descriptors, or syscall policy.

## Invariants and flows

Architecture initialization installs the exception callback and IDT before interrupts are enabled.
`roxy-interrupt` separately registers its interrupt dispatcher through the architecture contract,
then owns controller lifecycle and callback policy after entry dispatch. IRQ entry stubs only
normalize vectors; they do not acknowledge devices or route handlers themselves.
The x86_64 backend also enables x87/SSE execution, establishes the default FXSAVE state, and makes
that state type available to the thread subsystem. Long mode guarantees the required x87, FXSAVE,
and SSE2 capabilities.

Syscall entry normalizes the raw register frame, dispatches one handler, and restores the saved
userspace general-purpose registers when that handler returns, retaining `RAX` for the syscall
result. A fresh `execve` image uses the separate
`resume_user` contract so it can reset floating-point state and enter new RIP/RSP values without
returning through the old userspace frame.

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
