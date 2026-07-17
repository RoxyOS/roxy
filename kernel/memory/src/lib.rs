#![no_std]

extern crate alloc;

mod address;
pub mod frame;
mod heap;
mod mapper;
mod memory_map;
mod stats;

use core::sync::atomic::{AtomicBool, Ordering};

use roxy_boot::BootInfo;

use mapper::{CurrentMapper, Mapper};

pub use address::{PAGE_SIZE, PhysicalAddress, UserAddress, VirtualAddress};
pub use frame::{OwnedFrame, PageRef};
pub use stats::MemoryStats;

static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initializes physical memory, the active mapper, and the kernel heap.
///
/// # Panics
///
/// Panics if initialization is repeated or the boot memory layout is invalid or insufficient.
pub fn initialize(boot_info: &BootInfo) {
    assert!(
        !INITIALIZED.swap(true, Ordering::AcqRel),
        "memory initialized twice"
    );
    heap::initialize_bootstrap();

    let memory_map = memory_map::MemoryMap::from_boot_info(boot_info);
    frame::initialize(&memory_map.regions, boot_info.hhdm_offset);
    CurrentMapper::initialize(boot_info.hhdm_offset);
    heap::initialize_permanent();
}

#[must_use]
pub fn statistics() -> MemoryStats {
    let (total_frames, allocated_frames, frame_attempts) = frame::statistics();
    let (heap_total, heap_requested, heap_allocated, heap_attempts) = heap::statistics();

    MemoryStats {
        total_frames,
        allocated_frames,
        heap_total_bytes: heap_total,
        heap_requested_bytes: heap_requested,
        heap_allocated_bytes: heap_allocated,
        frame_allocation_attempts: frame_attempts,
        heap_allocation_attempts: heap_attempts,
    }
}
