use core::fmt;

use crate::device;

/// Initializes serial hardware and installs the unsupported-operation reporter.
pub fn initialize() {
    device::initialize();
    roxy_utils::unsupported::initialize(print);
}

/// Writes formatted diagnostics through the initialized serial device.
///
/// # Panics
///
/// Panics when the serial subsystem has not been initialized.
pub fn print(arguments: fmt::Arguments<'_>) {
    device::current().write_formatted(arguments);
}

pub fn emergency_print(arguments: fmt::Arguments<'_>) {
    device::emergency_write(arguments);
}
