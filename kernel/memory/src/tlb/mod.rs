#[cfg(target_arch = "x86_64")]
mod x86_64;

use crate::{PAGE_SIZE, VirtualAddress};

#[cfg(target_arch = "x86_64")]
use self::x86_64::X86_64Tlb;

#[cfg(target_arch = "x86_64")]
type CurrentTlbBackend = X86_64Tlb;

#[derive(Clone, Copy, Debug)]
pub enum TlbInvalidation {
    Page(VirtualAddress),
    Range {
        start: VirtualAddress,
        page_count: usize,
    },
    All,
}

/// Invalidates translations in the current CPU's active address space.
///
/// # Panics
///
/// Panics when a range is unaligned or its address calculation overflows.
pub fn invalidate(request: TlbInvalidation) {
    match request {
        TlbInvalidation::Page(address) => CurrentTlbBackend::invalidate_page(address),
        TlbInvalidation::Range { start, page_count } => invalidate_range(start, page_count),
        TlbInvalidation::All => CurrentTlbBackend::invalidate_all(),
    }
}

fn invalidate_range(start: VirtualAddress, page_count: usize) {
    assert_eq!(
        start.as_u64() % PAGE_SIZE,
        0,
        "TLB range is not page aligned"
    );

    for page in 0..page_count {
        let offset = u64::try_from(page).unwrap().checked_mul(PAGE_SIZE).unwrap();
        CurrentTlbBackend::invalidate_page(start.checked_add(offset).unwrap());
    }
}

trait TlbBackend: sealed::Sealed {
    fn invalidate_page(address: VirtualAddress);

    fn invalidate_all();
}

mod sealed {
    pub trait Sealed {}
}
