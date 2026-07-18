#![no_std]

extern crate alloc;

mod address;
pub mod frame;
mod heap;
mod mapper;
mod memory_map;
mod stats;
pub mod tlb;

use core::sync::atomic::{AtomicBool, Ordering};

use roxy_boot::BootInfo;

use mapper::{CurrentKernelPageTableBackend, KernelPageTableBackend, initialize_kernel_page_table};

pub use address::{PAGE_SIZE, PhysicalAddress, UserAddress, UserPage, VirtualAddress};
pub use frame::{FrameAccessError, OwnedFrame, PageRef};
pub use mapper::{
    AddrSpacePageTable, MappingError, PagePermissions, PageTableToken, activate_kernel_page_table,
    kernel_page_table_root,
};
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
    CurrentKernelPageTableBackend::initialize(boot_info.hhdm_offset);
    initialize_kernel_page_table();
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
