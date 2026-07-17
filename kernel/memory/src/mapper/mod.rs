mod x86_64;

use crate::{OwnedFrame, VirtualAddress};

pub(crate) use self::x86_64::X86_64Mapper;

pub(crate) type CurrentMapper = X86_64Mapper;

pub(crate) trait Mapper: sealed::Sealed {
    fn initialize(hhdm_offset: u64);

    fn is_mapped(address: VirtualAddress) -> bool;

    fn map_page(address: VirtualAddress, frame: OwnedFrame, flags: MappingFlags);
}

bitflags::bitflags! {
    #[derive(Clone, Copy)]
    pub(crate) struct MappingFlags: u8 {
        const WRITABLE = 1 << 0;
        const EXECUTABLE = 1 << 1;
        const USER = 1 << 2;
    }
}

mod sealed {
    pub trait Sealed {}
}
