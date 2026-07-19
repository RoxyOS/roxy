#![no_std]

pub mod lock;
pub mod preemption;
pub mod unsupported;

pub use lock::{Lock, LockGuard};
