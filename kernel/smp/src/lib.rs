//! SMP subsystem: starts the platform-parked application processors and owns the kernel-side
//! AP entry.

#![no_std]

mod ap;
mod arch;

/// Starts every application processor reported by the current architecture's platform, handing
/// each over to that backend's AP entry stub.
///
/// # Panics
///
/// Panics when a CPU cannot be registered.
///
/// # Safety
///
/// Each released AP immediately runs kernel code, so the caller must have no per-CPU data that an
/// AP's first visit to [`roxy_arch::current_cpu_id`] would read before the AP registers (the BSP
/// already claims slot 0). This is satisfied when called after the bootstrap processor's own
/// `roxy-cpu` state is initialized.
pub fn initialize() {
    arch::start_application_processors();
}
