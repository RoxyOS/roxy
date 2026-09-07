//! `x86_64` SMP bring-up: the bootloader parks secondary processors at boot, and the Limine MP
//! request releases them.

use limine::mp::MP_FLAG_X2APIC;
use limine::request::MpRequest;

/// Limine Multi-Processor request that parks application processors at boot and lets us release
/// them later.
#[used]
#[unsafe(link_section = ".limine_requests")]
static MP: MpRequest = MpRequest::new(MP_FLAG_X2APIC);

/// Starts every application processor reported by the bootloader, handing each over to
/// `ap_init`.
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
pub(crate) fn start_application_processors() {
    let Some(response) = MP.response() else {
        return;
    };

    for &cpu in response.cpus() {
        // The response includes the bootstrap processor; only non-bootstrap CPUs are started.
        if cpu.lapic_id == response.bsp_lapic_id {
            continue;
        }

        // `bootstrap` stores `extra_argument` (relaxed) then publishes `goto_addr` (release),
        // matching the Limine MP hand-over contract. The AP reads its own RSP on entry to
        // determine the kernel stack, so extra_argument is unused.
        cpu.bootstrap(ap_init, 0);
    }
}

/// The bootloader hand-over entry point for an application processor.
///
/// Runs on the bootloader-provided stack under the bootloader page tables, where nothing that
/// needs kernel heap or device mappings is available. It forwards to the shared
/// `crate::ap::ap_main_1`, which registers the AP's identity and switches it onto its own
/// kernel stack under the kernel page tables.
///
/// # Safety
///
/// The caller must invoke this only through the bootloader's per-CPU hand-over contract: on a
/// fresh AP, with interrupts disabled, on a bootloader-provided stack, and via a call that never
/// returns to the caller.
#[allow(clippy::missing_safety_doc)]
unsafe extern "C" fn ap_init(_info: &limine::mp::MpInfo) -> ! {
    crate::ap::ap_main_1()
}
