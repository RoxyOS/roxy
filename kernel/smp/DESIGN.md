# SMP Design

## Purpose and scope

`roxy-smp` owns the boot-time policy for bringing up multi-processing: it declares the bootloader's
MP request that parks application processors, releases those APs during boot, and owns the kernel
side of the AP entry. It does not own CPU identity, per-CPU descriptor tables, interrupt routing,
or the scheduler.

## Ownership and boundaries

- The Limine `MpRequest` static lives here (not in `roxy-boot`, which only parses boot data). The
  bootloader parks APs at boot and releases them when [`initialize`](crate::initialize) publishes
  each AP's entry point.
- [`initialize`](crate::initialize) runs on the BSP and calls `MpInfo::bootstrap` for every
  non-bootstrap CPU. `bootstrap` stores the extra argument (relaxed) then publishes the entry
  point (release), satisfying the bootloader's hand-over contract.
- The AP-side entry [`ap_main`](crate::ap_main) is the first kernel code each AP runs. It captures
  the current stack pointer (the bootloader's per-CPU stack), then delegates GDT/TSS/IDT setup to
  `roxy-arch` via `Architecture::initialize_application_processor`, and finally parks.
- Per-CPU GDT/TSS/IDT and CPU identity registration are owned by `roxy-arch`. The AP's hello-world
  diagnostic uses the blocking serial path so its output serialises with the BSP's instead of
  racing the UART's TX polling.

## Initialization flow

1. `roxy-boot` collects boot info (no MP data; `roxy-smp` holds the request).
2. `kernel-main` initialises CPU-local state on the BSP, then calls `roxy_smp::initialize()`.
3. `initialize` releases each parked AP with `ap_main` as the entry.
4. Each AP runs `ap_main`: captures its stack, registers CPU identity, sets up a per-CPU
   GDT/TSS and loads the shared IDT, prints `hello world`, then parks with interrupts disabled.
5. The BSP continues normal boot (timer, scheduler, init) while APs sit parked.

## Limits

- APs currently run under the bootloader's page tables (the BSP switched to its own during memory
  init), own no per-CPU local-APIC/timer, and the scheduler is BSP-only. They park with interrupts
  disabled and must not touch kernel heap or device mappings. A real per-CPU idle loop requires
  per-CPU local-APIC/timer setup, the kernel page tables on each AP, and a scheduler that can
  migrate threads — tracked as a TODO in `ap_main`.
- In test (`kernel-test`) builds `initialize` is not called, so APs are never released and cannot
  interfere with the serial-based test harness.