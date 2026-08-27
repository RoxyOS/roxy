#![no_std]

extern crate alloc;

mod convert;
mod device;

pub use device::FramebufferDevice;

use alloc::sync::Arc;

use roxy_devfs::DeviceRegistry;

/// Registers the boot framebuffer as `/dev/fb0` when the framebuffer terminal initialized it.
///
/// Serial-only or unsupported-mode boots publish no layout and register no device.
///
/// # Panics
///
/// Panics when another device already registered the `fb0` path.
pub fn register(registry: &DeviceRegistry) {
    let Some(layout) = roxy_fbterm::framebuffer_layout() else {
        return;
    };

    registry
        .register(b"fb0", Arc::new(FramebufferDevice::new(layout)))
        .expect("fb0 is registered exactly once");
}
