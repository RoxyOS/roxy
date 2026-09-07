//! Architecture-specific application-processor discovery and release.
//!
//! `roxy-smp` owns the *policy* of bringing up secondary processors, but the two things that
//! genuinely differ across architectures stay behind this seam: where the secondary CPU list comes
//! from and the mechanism that starts each one (on `x86_64` the bootloader parks them and the Limine
//! MP request releases them), plus the hand-over entry stub's signature. Every backend's stub
//! forwards to the shared bring-up sequence in `crate::ap::ap_main_1`, so backends never
//! duplicate the bring-up order.

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub(crate) use self::x86_64::start_application_processors;
