mod allocator;
mod reference;

pub use reference::{OwnedFrame, PageRef};

use crate::memory_map::MemoryRegion;

pub(crate) use allocator::{allocate as allocate_raw, statistics};

#[must_use]
pub fn allocate() -> Option<OwnedFrame> {
    allocate_raw().map(OwnedFrame::new)
}

pub(crate) fn initialize(regions: &[MemoryRegion], hhdm_offset: u64) {
    allocator::initialize(regions, hhdm_offset);
}
