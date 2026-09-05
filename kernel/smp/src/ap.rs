//! Application-processor entry: the first kernel code each non-bootstrap CPU runs after the
//! bootloader releases it.

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_serial::s_println;

/// The bootloader hand-over entry point for an application processor.
///
/// Runs on the bootloader-provided stack under the bootloader page tables, where nothing that
/// needs kernel heap or device mappings is available. It registers the AP's identity, sets up its
/// per-CPU GDT/TSS/IDT, then switches onto the AP's own kernel stack under the kernel page tables
/// and hands control to [`ap_main_2`].
///
/// # Safety
///
/// The caller must invoke this only through the bootloader's per-CPU hand-over contract: on a
/// fresh AP, with interrupts disabled, on a bootloader-provided stack, and via a call that never
/// returns to the caller.
#[allow(clippy::missing_safety_doc)]
pub unsafe extern "C" fn ap_main_1(_info: &limine::mp::MpInfo) -> ! {
    CurrentArchitectureBackend::register_application_processor();
    let cpu_id = CurrentArchitectureBackend::current_cpu_id();
    let kernel_stack_top = CurrentArchitectureBackend::ap_kernel_stack_top(cpu_id);

    CurrentArchitectureBackend::initialize_application_processor(kernel_stack_top);

    let page_table_root = roxy_memory::kernel_page_table_root().as_u64();
    unsafe {
        CurrentArchitectureBackend::switch_stack_pt_and_call(
            kernel_stack_top,
            page_table_root,
            ap_main_2,
        )
    }
}

/// Runs on the AP's own kernel stack under the kernel page tables.
///
/// Everything after the stack/page-table switch is ordinary kernel code: it can use the heap and
/// device mappings, so this is also where the AP would enter a per-CPU scheduler idle loop in a
/// later phase.
extern "C" fn ap_main_2() -> ! {
    let cpu_id = CurrentArchitectureBackend::current_cpu_id();
    s_println!("AP main on cpu {cpu_id}: hello world");

    roxy_interrupt::initialize_ap();

    loop {
        CurrentArchitectureBackend::wait_for_interrupt();
    }
}
