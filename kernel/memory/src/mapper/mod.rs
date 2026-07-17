mod pagetable;
#[cfg(target_arch = "x86_64")]
mod x86_64;

use crate::{OwnedFrame, VirtualAddress};

#[cfg(target_arch = "x86_64")]
use self::x86_64::X86_64KernelPageTableBackend;
pub use pagetable::{AddrSpacePageTable, MappingError, PagePermissions, PageTableToken};

#[cfg(target_arch = "x86_64")]
pub(crate) type CurrentKernelPageTableBackend = X86_64KernelPageTableBackend;

pub(crate) trait KernelPageTableBackend: sealed::Sealed {
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
