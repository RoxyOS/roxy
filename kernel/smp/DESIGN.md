# SMP Design

## Purpose and scope

`roxy-smp` owns the boot-time policy for bringing up multi-processing: it owns the kernel side of
AP entry and delegates secondary-CPU discovery and release to a per-architecture seam. It does not
own CPU identity, per-CPU descriptor tables, interrupt routing, or the scheduler.

## Ownership and boundaries

- Secondary-CPU discovery and release live behind `src/arch/`, the same cfg-selected seam as
  `roxy-interrupt` and `roxy-memory`. The x86_64 backend owns the Limine `MpRequest` static (which
  parks APs at boot and is released by `arch::start_application_processors`) and the hand-over
  entry stub `ap_init`. Another architecture would own its platform mechanism (e.g. PSCI)
  here instead.
- [`initialize`](crate::initialize) is the common BSP-side entry; it delegates directly to
  `arch::start_application_processors`. On x86_64 that calls `MpInfo::bootstrap` for every
  non-bootstrap CPU.
- The AP-side bring-up is split into a shared, architecture-neutral sequence and thin per-backend
  stubs. `ap::ap_main_1` runs the common order: register the CPU's identity, pick the per-CPU
  kernel stack, delegate GDT/TSS/IDT (or the aarch64 analogue) setup to `roxy-arch` via
  `Architecture::initialize_application_processor`, then switch onto the AP's own kernel stack
  under the kernel page tables into `ap_main_2`. Each backend's entry stub only forwards here (the
  x86_64 stub is `ap_init(info) -> ap_main_1()`); `ap_main_2` initialises the local interrupt
  controller, scheduler slot, and per-CPU timer, then enters the scheduler control loop.
- Per-CPU GDT/TSS/IDT and CPU identity registration are owned by `roxy-arch`. The AP's hello-world
  diagnostic uses the blocking serial path so its output serialises with the BSP's instead of
  racing the UART's TX polling.

## Initialization flow

1. `roxy-boot` collects boot info (no MP data; `roxy-smp` holds the request).
2. `kernel-main` initialises CPU-local state on the BSP, calibrates the periodic timer (populating
   the shared LAPIC timer count), registers the scheduler's timer handler, then calls
   `roxy_smp::initialize()`. `kernel-main` also gates the AP-dispatch readiness signal early: the
   scheduler lets APs take runnable threads only after the initial process has been spawned, so
   they never steal the still-booting thread during the fragile startup window.
3. `initialize` delegates to `arch::start_application_processors`, which releases each parked AP
   with the backend's entry stub (on x86_64, `ap_init`) as the entry.
4. Each released AP runs the backend's entry stub, which forwards to `ap_main_1` on the
   bootloader-provided stack and page tables: registers CPU identity, selects its per-CPU kernel
   stack, sets up a per-CPU GDT/TSS and loads the shared IDT, then switches onto its own kernel
   stack under the kernel page tables into `ap_main_2`. There it prints `hello world`, enables its
   local interrupt controller, initialises its `LocalScheduler` slot and periodic timer, and enters
   the scheduler control loop (`roxy-thread`), idling until the BSP raises the AP-dispatch readiness
   signal.
5. The BSP continues normal boot (rest of init, userspace) while APs idle in `wait-for-interrupt`
   behind the readiness signal, then run their own scheduler control loops once `kernel-main`
   raises it after spawning the initial process.

## Application-processor bring-up

Each AP's bring-up runs first on the bootloader-provided stack under the bootloader page tables
(the parts that need no kernel heap or device mappings), then switches permanently onto a
kernel-`.bss` per-CPU stack under the kernel page tables via `roxy-arch`. The per-CPU kernel stack
lives in the kernel image's `.bss`, so it is mapped under both the bootloader and kernel page
tables, letting the AP switch stacks before switching CR3.

## Limits

- The BSP calibrates the periodic timer once and shares the resulting LAPIC initial count with the
  APs, so only the BSP touches the shared PIT. APs reuse the shared count and never calibrate.
- Application processors are held behind an `APS_READY` gate until the BSP has spawned the initial
  process, so they cannot steal the boot thread or run userspace before devices and the process
  table are ready. Each AP then dispatches runnable threads and runs its userspace on its own
  kernel stack. A shared `IDLE` array plus a reschedule IPI wakes a free AP immediately when a
  thread becomes runnable, instead of waiting for the next timer tick.
- In test (`kernel-test`) builds `initialize` is not called (the call is gated out in `kernel-main`),
  so APs are never released and cannot interfere with the serial-based test harness.