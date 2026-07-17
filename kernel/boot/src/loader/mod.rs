mod limine;

use crate::BootInfo;

pub use self::limine::Limine;

pub type CurrentLoader = Limine;

pub trait Bootloader: sealed::Sealed {
    #[must_use]
    fn parse() -> BootInfo;
}

mod sealed {
    pub trait Sealed {}
}
