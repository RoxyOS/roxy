#![no_std]

extern crate alloc;

mod filesystem;
mod registry;

pub use filesystem::DevFs;
pub use registry::{Device, DeviceRegistry, RegisterError};
