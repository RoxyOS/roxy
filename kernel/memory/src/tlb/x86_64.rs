use x86_64::{VirtAddr, instructions::tlb};

use crate::VirtualAddress;

use super::{TlbBackend, sealed};

pub(super) struct X86_64Tlb;

impl sealed::Sealed for X86_64Tlb {}

impl TlbBackend for X86_64Tlb {
    fn invalidate_page(address: VirtualAddress) {
        tlb::flush(VirtAddr::new(address.as_u64()));
    }

    fn invalidate_all() {
        tlb::flush_all();
    }
}
