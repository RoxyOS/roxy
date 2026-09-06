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

## Application-processor bring-up

AP bring-up is split so an AP can resolve its `CpuId` and pick its per-CPU kernel stack before
tables are installed:

- `register_application_processor` claims the CPU in the identity map (so `current_cpu_id`
  resolves earlier than `initialize_application_processor`).
- `ap_kernel_stack_top` returns the top of the AP's dedicated kernel-`.bss` stack (mapped under
  both bootloader and kernel page tables).
- `initialize_application_processor` builds a per-CPU GDT (with a per-CPU TSS entry over shared
  code/data/user descriptors), loads the global IDT, configures the TSS RSP0/syscall kernel
  stack, and programs this CPU's syscall MSRs (`EFER.SCE`, `IA32_STAR`/`LSTAR`/`SFMASK`).
- `switch_stack_pt_and_call` loads the kernel page tables, switches onto the per-CPU kernel
  stack, and enters an interrupt-enabled idle loop; it never returns.

IDT and the standard segment descriptors are shared; per-CPU GDT, TSS, dual-fault stack, and kernel
stack state live in `AP_*` arrays indexed by `CpuId`, each written once by its owning CPU with
interrupts disabled and read before any heap or device mapping is touched (the bring-up before the
switch must not use the kernel heap).

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
stack through the architecture boundary. The x86_64 backend records it in the current CPU's
per-CPU syscall slot and in that CPU's TSS RSP0 while interrupts are disabled, so syscalls,
interrupts, and exceptions from ring 3 all enter on the same thread-owned stack.

The syscall entry and its `RSP0` bookkeeping are per-CPU because any CPU can service a syscall
concurrently. Each CPU owns a `SyscallEntryState` slot (kernel stack top, the user-`RSP` handoff the
naked entry builds its frame from, and this CPU's TSS address), and sets its `GS.base` to that
slot during bring-up. The naked `entry` reaches the slot through `gs:` operands; the syscall MSRs
are programmed on every active CPU for the same reason.

`GS` is reserved for this kernel per-CPU area. `CR4.FSGSBASE` stays clear so userspace cannot run
`wrgsbase`/`rdgsbase`, and supported userspaces never load a data selector into `GS`; were one to,
loading a flat 64-bit data segment zeroes the base and would break the next syscall entry.

## Failure and limits

Invalid CPU state, unsupported exception forms, non-canonical user pointers, and repeated
initialization are kernel faults rather than recoverable userspace errors. The backend currently
assumes the BSP-oriented CPU model exposed by the rest of the kernel and provides no portability
promise beyond the implemented target.

An unrecoverable failure stops the whole machine, not just the faulting core. The first core to
fail (the panic handler or the unrecoverable-exception path) asks `roxy-interrupt` to broadcast a
stop NMI to every other CPU and then halts; each peer receives that NMI through the `NonMaskable`
exception vector (architecturally fixed to IDT slot 2) and halts without re-entering the panic
path or taking seqlocks, so a peer that was interrupted inside a critical section is not torn.
NMI delivery ignores the target `IF`, which is what guarantees even a peer running with interrupts
disabled is stopped.

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
