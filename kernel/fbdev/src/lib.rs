#![no_std]

extern crate alloc;

mod claim;
mod convert;
mod device;

pub use device::FramebufferDevice;

use alloc::sync::Arc;

use roxy_devfs::DeviceRegistry;

/// Registers the boot framebuffer as `/dev/framebuffer` when the framebuffer terminal
/// initialized it.
///
/// Serial-only or unsupported-mode boots publish no layout, register no device, and register no
/// ownership notification, because they have neither a visible frame to hand over nor a terminal
/// that draws on one.
///
/// # Panics
///
/// Panics when another device already registered the `framebuffer` path.
pub fn register(registry: &DeviceRegistry) {
    let Some(layout) = roxy_fbterm::framebuffer_layout() else {
        return;
    };

    registry
        .register(b"framebuffer", Arc::new(FramebufferDevice::new(layout)))
        .expect("framebuffer is registered exactly once");

    roxy_process::register_process_exit_handler(claim::release_exited);
}
