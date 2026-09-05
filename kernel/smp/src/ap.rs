//! Application-processor entry: the first kernel code each non-bootstrap CPU runs after the
//! bootloader releases it.

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_serial::e_println;

/// The kernel entry point for an application processor.
///
/// The bootloader calls this on the fresh AP with interrupts disabled, on a dedicated stack, and
/// with the same longer machine state as the bootstrap processor (including valid 64-bit
/// segments). The function returns `!` because an AP continues running kernel code.
///
/// # Safety
///
/// The caller must invoke this only through the bootloader's per-CPU hand-over contract: on a
/// fresh AP, with interrupts disabled, on a bootloader-provided stack, and via a call that never
/// returns to the caller.
pub unsafe extern "C" fn ap_main() -> ! {
    let cpu_id = CurrentArchitectureBackend::initialize_application_processor();

    e_println!("AP main on cpu {cpu_id}: hello world");

    // TODO(smp-worker): Parking is a stopgap until APs can do real work. Each AP currently runs
    // under the bootloader's page tables (the BSP switched to its own after memory init), owns no
    // per-CPU GDT/TSS, and the scheduler is BSP-only, so the parked loop must keep interrupts
    // disabled and never touch kernel heap or device mappings. A real per-CPU idle loop requires
    // a per-CPU GDT/TSS, the kernel page tables on this CPU, a per-CPU local-APIC/timer setup, and
    // a scheduler that can migrate threads here.
    CurrentArchitectureBackend::halt_forever()
}
