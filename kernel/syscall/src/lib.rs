#![no_std]

mod dispatch;
mod exit;

use roxy_arch::{Architecture, CurrentArchitectureBackend};

pub fn initialize() {
    dispatch::validate_registry();
    CurrentArchitectureBackend::configure_syscall(dispatch::dispatch);
}
