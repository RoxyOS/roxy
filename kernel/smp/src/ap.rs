//! Application-processor entry: the first kernel code each non-bootstrap CPU runs after the
//! bootloader releases it.

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_serial::s_println;

/// The kernel entry point for an application processor.
///
/// # Safety
///
/// The caller must invoke this only through the bootloader's per-CPU hand-over contract: on a
/// fresh AP, with interrupts disabled, on a bootloader-provided stack, and via a call that never
/// returns to the caller.
pub unsafe extern "C" fn ap_main(_info: &limine::mp::MpInfo) -> ! {
    let kernel_stack_top = CurrentArchitectureBackend::current_stack_pointer();
    CurrentArchitectureBackend::initialize_application_processor(kernel_stack_top);
    let cpu_id = CurrentArchitectureBackend::current_cpu_id();

    s_println!("AP main on cpu {cpu_id}: hello world\n");

    // TODO(smp-worker): Parking is a stopgap until APs can do real work. Each AP currently runs
    // under the bootloader's page tables (the BSP switched to its own after memory init), owns no
    // per-CPU local APIC/timer, and the scheduler is BSP-only, so the parked loop must keep
    // interrupts disabled and never touch kernel heap or device mappings. A real per-CPU idle
    // loop requires a per-CPU local-APIC/timer, the kernel page tables on this CPU, and a
    // scheduler that can migrate threads here.
    CurrentArchitectureBackend::halt_forever()
}
