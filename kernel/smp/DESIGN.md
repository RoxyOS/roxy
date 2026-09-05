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
- The AP-side entry [`ap_main_1`](crate::ap_main_1) is the first kernel code each AP runs, on the
  bootloader-provided stack and page tables. It registers the CPU's identity, delegates GDT/TSS/IDT
  setup to `roxy-arch` via `Architecture::initialize_application_processor`, then switches onto the
  AP's own kernel stack under the kernel page tables into `ap_main_2`, which parks.
- Per-CPU GDT/TSS/IDT and CPU identity registration are owned by `roxy-arch`. The AP's hello-world
  diagnostic uses the blocking serial path so its output serialises with the BSP's instead of
  racing the UART's TX polling.

## Initialization flow

1. `roxy-boot` collects boot info (no MP data; `roxy-smp` holds the request).
2. `kernel-main` initialises CPU-local state on the BSP, then calls `roxy_smp::initialize()`.
3. `initialize` releases each parked AP with [`ap_main_1`](crate::ap_main_1) as the entry.
4. Each AP runs `ap_main_1` on the bootloader-provided stack and page tables: registers CPU
   identity, selects its per-CPU kernel stack, sets up a per-CPU GDT/TSS and loads the shared IDT,
   then switches onto its own kernel stack under the kernel page tables into `ap_main_2`. There it
   prints `hello world`, enables its local APIC, and halts in an interrupt-enabled idle loop.
5. The BSP continues normal boot (timer, scheduler, init) while APs sit halted.

## Application-processor bring-up

Each AP's bring-up runs first on the bootloader-provided stack under the bootloader page tables
(the parts that need no kernel heap or device mappings), then switches permanently onto a
kernel-`.bss` per-CPU stack under the kernel page tables via `roxy-arch`. The per-CPU kernel stack
lives in the kernel image's `.bss`, so it is mapped under both the bootloader and kernel page
tables, letting the AP switch stacks before switching CR3.

## Limits

- The periodic timer is not yet set up on APs (`TODO(smp-timer)`): its LAPIC calibration uses the
  shared PIT, so only one CPU may calibrate at a time. Once a scheduler exists, either the BSP
  calibrates once and shares the count, or the PIT use is serialised.
- APs halt with interrupts enabled but no timer and no IRQ routing, so they idle without preemption
  until the scheduler is per-CPU. They can be woken by an IPI (Phase B).
- In test (`kernel-test`) builds `initialize` is not called, so APs are never released and cannot
  interfere with the serial-based test harness.