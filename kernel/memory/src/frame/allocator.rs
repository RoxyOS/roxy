#[cfg(debug_assertions)]
use alloc::collections::BTreeSet;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use buddy_system_allocator::FrameAllocator;
use roxy_boot::MemoryRegionKind;
use roxy_utils::Lock;
use spin::Once;

use crate::{PhysicalAddress, address::PAGE_SIZE, memory_map::MemoryRegion};

const FRAME_ORDER: usize = 40;
#[cfg(debug_assertions)]
const ALLOCATED_POISON: u8 = 0xaa;
#[cfg(not(debug_assertions))]
const ALLOCATED_POISON: u8 = 0;
#[cfg(debug_assertions)]
const FREED_POISON: u8 = 0xdd;
#[cfg(not(debug_assertions))]
const FREED_POISON: u8 = 0;

static ALLOCATOR: Once<Lock<PhysicalFrameAllocator>> = Once::new();
static HHDM_OFFSET: AtomicU64 = AtomicU64::new(0);
static ALLOCATION_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy)]
pub(super) struct FrameIndex(usize);

impl FrameIndex {
    pub(super) fn start_address(self) -> PhysicalAddress {
        let address = u64::try_from(self.0)
            .unwrap()
            .checked_mul(PAGE_SIZE)
            .unwrap();
        PhysicalAddress::new(address).unwrap()
    }
}

struct PhysicalFrameAllocator {
    allocator: FrameAllocator<FRAME_ORDER>,
    total: usize,
    allocated: usize,
    #[cfg(debug_assertions)]
    live: BTreeSet<usize>,
}

impl PhysicalFrameAllocator {
    fn from_regions(regions: &[MemoryRegion]) -> Self {
        let mut allocator = Self {
            allocator: FrameAllocator::new(),
            total: 0,
            allocated: 0,
            #[cfg(debug_assertions)]
            live: BTreeSet::new(),
        };

        for region in regions
            .iter()
            .filter(|region| region.kind == MemoryRegionKind::Usable)
        {
            allocator.add_region(*region);
        }

        allocator
    }

    fn add_region(&mut self, region: MemoryRegion) {
        let start = usize::try_from(region.start / PAGE_SIZE).unwrap();
        let end = usize::try_from(region.end / PAGE_SIZE).unwrap();
        self.allocator.add_frame(start, end);
        self.total = self.total.checked_add(end - start).unwrap();
    }

    fn allocate(&mut self) -> Option<FrameIndex> {
        let frame = self.allocator.alloc(1)?;
        self.allocated = self.allocated.checked_add(1).unwrap();

        #[cfg(debug_assertions)]
        assert!(self.live.insert(frame), "frame allocated twice");

        Some(FrameIndex(frame))
    }

    fn deallocate(&mut self, frame: FrameIndex) {
        #[cfg(debug_assertions)]
        assert!(self.live.remove(&frame.0), "invalid frame deallocation");

        self.allocated = self.allocated.checked_sub(1).unwrap();
        self.allocator.dealloc(frame.0, 1);
    }
}

pub(crate) fn initialize(regions: &[MemoryRegion], hhdm_offset: u64) {
    assert!(
        !ALLOCATOR.is_completed(),
        "frame allocator initialized twice"
    );
    HHDM_OFFSET.store(hhdm_offset, Ordering::Release);
    ALLOCATOR.call_once(|| Lock::new(PhysicalFrameAllocator::from_regions(regions)));
}

pub(super) fn allocate() -> Option<FrameIndex> {
    ALLOCATION_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    let frame = ALLOCATOR.get().unwrap().lock().allocate()?;
    poison(frame, ALLOCATED_POISON);
    Some(frame)
}

pub(super) fn deallocate(frame: FrameIndex) {
    poison(frame, FREED_POISON);
    ALLOCATOR.get().unwrap().lock().deallocate(frame);
}

pub(crate) fn physical_pointer<T>(address: PhysicalAddress) -> *mut T {
    let virtual_address = HHDM_OFFSET
        .load(Ordering::Acquire)
        .checked_add(address.as_u64())
        .unwrap();
    usize::try_from(virtual_address).unwrap() as *mut T
}

pub(crate) fn hhdm_offset() -> u64 {
    HHDM_OFFSET.load(Ordering::Acquire)
}

pub(crate) fn statistics() -> (usize, usize, usize) {
    let Some(allocator) = ALLOCATOR.get() else {
        return (0, 0, ALLOCATION_ATTEMPTS.load(Ordering::Relaxed));
    };
    let allocator = allocator.lock();
    (
        allocator.total,
        allocator.allocated,
        ALLOCATION_ATTEMPTS.load(Ordering::Relaxed),
    )
}

#[cfg(debug_assertions)]
fn poison(frame: FrameIndex, value: u8) {
    let pointer = physical_pointer::<u8>(frame.start_address());

    // SAFETY: The HHDM maps this allocated frame, and the caller serializes ownership changes.
    unsafe { pointer.write_bytes(value, usize::try_from(PAGE_SIZE).unwrap()) };
}

#[cfg(not(debug_assertions))]
const fn poison(_frame: FrameIndex, _value: u8) {}
