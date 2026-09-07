//! Application-processor entry: the first kernel code each non-bootstrap CPU runs after the
//! platform releases it.

use roxy_arch::{Architecture, CurrentArchitectureBackend};
use roxy_serial::s_println;

/// The common, architecture-neutral bring-up every released application processor runs before it
/// can run ordinary kernel code: the first phase of AP boot, running on the backend's provided
/// stack and page tables.
///
/// It registers the AP's identity, sets up its per-CPU descriptor tables / syscall state, then
/// switches onto the AP's own kernel stack under the kernel page tables and hands control to
/// [`ap_main_2`]. The step order lets an AP resolve its `CpuId` and pick its kernel stack before
/// tables are installed and before any heap or device mapping is touched.
///
/// Each architecture backend's hand-over entry stub in `crate::arch` forwards here, so backends
/// differ only in the hand-over signature and how they get released, never in this bring-up order.
pub(crate) fn ap_main_1() -> ! {
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
/// Everything after the stack/page-table switch is ordinary kernel code: it initialises the
/// per-CPU local APIC, scheduler slot, and timer, then enters the scheduler control loop.
extern "C" fn ap_main_2() -> ! {
    let cpu_id = CurrentArchitectureBackend::current_cpu_id();
    s_println!("AP main on cpu {cpu_id}: hello world");

    roxy_interrupt::initialize_ap();
    roxy_thread::scheduler::initialize_local();
    roxy_time::initialize_ap_timer();
    roxy_time::start_periodic_timer();
    roxy_thread::scheduler::start();
}
