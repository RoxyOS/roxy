//! SMP subsystem: starts the bootloader-parked application processors and owns the kernel-side
//! AP entry.

#![no_std]

mod ap;

use limine::mp::MP_FLAG_X2APIC;
use limine::mp::MpGotoFunction;
use limine::request::MpRequest;

pub use ap::ap_main;

/// Limine Multi-Processor request that parks application processors at boot and lets us release
/// them later.
#[used]
#[unsafe(link_section = ".limine_requests")]
static MP: MpRequest = MpRequest::new(MP_FLAG_X2APIC);

/// Starts every application processor reported by the bootloader, handing each over to
/// [`ap_main`].
///
/// # Panics
///
/// Panics when the bootloader returns no MP response (the request must be present) or when a CPU
/// cannot be registered.
///
/// # Safety
///
/// Each released AP immediately runs kernel code, so the caller must have no per-CPU data that an
/// AP's first visit to [`roxy_arch::current_cpu_id`] would read before the AP registers (the BSP
/// already claims slot 0). This is satisfied when called after the bootstrap processor's own
/// `roxy-cpu` state is initialized.
pub fn initialize() {
    let Some(response) = MP.response() else {
        panic!()
    };

    for &cpu in response.cpus() {
        // The response includes the bootstrap processor; only non-bootstrap CPUs are started.
        if cpu.lapic_id == response.bsp_lapic_id {
            continue;
        }

        let entry: MpGotoFunction =
            unsafe { core::mem::transmute(ap_main as unsafe extern "C" fn() -> !) };

        cpu.bootstrap(entry, 0);
    }
}
