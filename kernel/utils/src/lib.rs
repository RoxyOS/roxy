#![no_std]

pub mod lock;
pub mod preemption;

pub use lock::{Lock, LockGuard};
