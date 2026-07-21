#![no_std]

extern crate alloc;

mod device;
mod logging;
mod terminal;

pub use logging::initialize;
#[doc(hidden)]
pub use logging::{emergency_print, print};
pub use terminal::terminal;

#[macro_export]
macro_rules! e_println {
    () => {
        $crate::emergency_print(format_args!("\n"))
    };
    ($($arguments:tt)*) => {
        $crate::emergency_print(format_args!("{}\n", format_args!($($arguments)*)))
    };
}
