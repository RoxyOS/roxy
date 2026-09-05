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
- `CpuId` is a logical slot index (`0..n-1`) into per-CPU storage arrays (`CpuLocal`, indexed by
  `current_cpu_id`). The architecture layer maps the physical CPU identity (the APIC id on x86)
  to a densely numbered `CpuId` through a fixed registration table. The x86_64 backend reads the
  current CPU's APIC id via CPUID leaf 1 (`EBX[31:24]`, Initial APIC ID) so that identity is
  available before x2APIC mode is enabled. BSP registration happens during `Architecture::initialize`,
  before any kernel code queries per-CPU storage; application processors register as their first
  kernel action by calling the backend CPU map directly.
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
userspace general-purpose registers when that handler returns. The handler returns a `SyscallExit` (`Returned`, `Resume`, or `RestoreContext`)
instead of a bare value: `value` is written back as the syscall result, an optional `redirect`
rewrites the saved instruction/stack pointers and argument registers (signal delivery), and an
optional `restore` replaces the entire saved context (`sigreturn`). A fresh `execve` image uses
the separate
`resume_user` contract so it can reset floating-point state and enter new RIP/RSP values without
returning through the old userspace frame.

Architecture methods that enter userspace require the active page table to map the supplied user
addresses. Unsafe backend code must keep this obligation local and document it at the call site.

Before dispatching a user thread, the scheduler supplies the top of that thread's owned kernel
stack through the architecture boundary. The x86_64 backend updates both the software SYSCALL
entry stack and TSS RSP0 while interrupts are disabled, so syscalls, interrupts, and exceptions
from ring 3 all enter on the same thread-owned stack.

## Failure and limits

Invalid CPU state, unsupported exception forms, non-canonical user pointers, and repeated
initialization are kernel faults rather than recoverable userspace errors. The backend currently
assumes the BSP-oriented CPU model exposed by the rest of the kernel and provides no portability
promise beyond the implemented target.

## Design decisions
- The trait exposes semantic operations such as `resume_user` and `set_kernel_stack_top` rather than
  register-level helpers. This keeps syscall and process code independent of x86 register names and
  prevents architecture details from leaking into generic subsystems.
- The CPU identity map uses a fixed `[(u32, CpuId); MAX_CPUS]` array with a linear scan on every
  `current_cpu_id` call. This avoids allocation and a hasher in the per-CPU hot path, keeps the
  `roxy-arch` crate free of an allocator dependency, and guarantees deterministic slot assignment.
  The key is the CPUID Initial APIC ID rather than the x2APIC MSR because the map must be readable
  before x2APIC mode is enabled. On the current single-vCPU platform the two are identical; on
  future SMP the post-enable x2APIC linear id may differ from the Initial APIC ID, and matching
  the map key to the hardware id consumed by the interrupt backend (`local_apic.id()`) is an open
  TODO.
