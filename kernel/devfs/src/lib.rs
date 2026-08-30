#![no_std]

extern crate alloc;

mod filesystem;
mod null;
mod registry;

pub use filesystem::DevFs;
pub use null::{NullDevice, register_null};
pub use registry::{Device, DeviceRegistry, RegisterError};
