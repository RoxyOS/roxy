#![no_std]

extern crate alloc;

mod device;
mod file;
mod kernel_terminal;

pub use device::TerminalDevice;
pub use file::open;
#[doc(hidden)]
pub use kernel_terminal::print;
pub use kernel_terminal::{kernel_terminal, select_kernel_terminal};

#[macro_export]
macro_rules! print {
    ($($arguments:tt)*) => {
        $crate::print(format_args!($($arguments)*))
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print(format_args!("\n"))
    };
    ($($arguments:tt)*) => {
        $crate::print(format_args!("{}\n", format_args!($($arguments)*)))
    };
}
