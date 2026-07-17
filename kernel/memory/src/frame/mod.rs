mod allocator;
mod reference;

pub use reference::{FrameAccessError, OwnedFrame, PageRef};

use crate::memory_map::MemoryRegion;

pub(crate) use allocator::statistics;
pub(crate) use allocator::{hhdm_offset, physical_pointer};

#[must_use]
pub fn allocate() -> Option<OwnedFrame> {
    allocator::allocate().map(OwnedFrame::new)
}

#[must_use]
pub fn allocate_zeroed() -> Option<PageRef> {
    let frame = allocate()?;
    frame.zero();
    Some(frame.into_page_ref())
}

pub(crate) fn initialize(regions: &[MemoryRegion], hhdm_offset: u64) {
    allocator::initialize(regions, hhdm_offset);
}
