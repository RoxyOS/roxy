#![no_std]

extern crate alloc;

mod device;
mod kernel_terminal;
mod tee;

pub use device::{OutputError, TerminalOutput};
#[doc(hidden)]
pub use kernel_terminal::print;
pub use kernel_terminal::{kernel_terminal, select_kernel_terminal};
pub use tee::TeeOutput;

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
