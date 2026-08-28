use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    ptr,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use buddy_system_allocator::Heap;
use roxy_utils::Lock;

use crate::{
    PAGE_SIZE, VirtualAddress, frame,
    mapper::{CurrentKernelPageTableBackend, KernelPageTableBackend, MappingFlags},
};

const BOOTSTRAP_HEAP_SIZE: usize = 256 * 1024;
const HEAP_ORDER: usize = 32;
const PERMANENT_HEAP_SIZE: usize = 64 * 1024 * 1024;

unsafe extern "C" {
    static __kernel_end: u8;
}

#[repr(C, align(4096))]
struct BootstrapHeap([u8; BOOTSTRAP_HEAP_SIZE]);

struct BootstrapCell(UnsafeCell<BootstrapHeap>);

// SAFETY: The bootstrap range is accessed only while initializing the locked global allocator.
unsafe impl Sync for BootstrapCell {}

static BOOTSTRAP_HEAP: BootstrapCell =
    BootstrapCell(UnsafeCell::new(BootstrapHeap([0; BOOTSTRAP_HEAP_SIZE])));

#[cfg_attr(target_os = "none", global_allocator)]
static GLOBAL_ALLOCATOR: KernelAllocator = KernelAllocator::new();

struct KernelAllocator {
    heap: Lock<Heap<HEAP_ORDER>>,
    initialized: AtomicBool,
    attempts: AtomicUsize,
}

impl KernelAllocator {
    const fn new() -> Self {
        Self {
            heap: Lock::new(Heap::empty()),
            initialized: AtomicBool::new(false),
            attempts: AtomicUsize::new(0),
        }
    }

    fn initialize_bootstrap(&self) {
        assert!(!self.initialized.swap(true, Ordering::AcqRel));

        let start = BOOTSTRAP_HEAP.0.get().cast::<u8>() as usize;
        // SAFETY: The static bootstrap range is unique, writable, and added exactly once.
        unsafe { self.heap.lock().init(start, BOOTSTRAP_HEAP_SIZE) };
    }

    unsafe fn add_range(&self, start: usize, size: usize) {
        assert!(self.initialized.load(Ordering::Acquire));
        // SAFETY: The caller guarantees the mapped range is unique and permanently writable.
        unsafe { self.heap.lock().init(start, size) };
    }
}

// SAFETY: All heap mutation is serialized, and registered ranges remain valid permanently.
unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.attempts.fetch_add(1, Ordering::Relaxed);

        if !self.initialized.load(Ordering::Acquire) {
            return ptr::null_mut();
        }

        let allocation = self.heap.lock().alloc(layout).ok();
        let pointer = allocation.map_or(ptr::null_mut(), core::ptr::NonNull::as_ptr);

        #[cfg(debug_assertions)]
        if !pointer.is_null() {
            // SAFETY: The allocator returned at least layout.size() writable bytes.
            unsafe { pointer.write_bytes(0xaa, layout.size()) };
        }

        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        #[cfg(debug_assertions)]
        // SAFETY: GlobalAlloc requires this pointer and layout to identify a live allocation.
        unsafe {
            pointer.write_bytes(0xdd, layout.size());
        }

        // SAFETY: GlobalAlloc requires this pointer and layout to match a live allocation.
        unsafe {
            self.heap
                .lock()
                .dealloc(core::ptr::NonNull::new_unchecked(pointer), layout);
        }
    }
}

pub(crate) fn initialize_bootstrap() {
    GLOBAL_ALLOCATOR.initialize_bootstrap();
}

unsafe fn add_permanent_range(start: usize, size: usize) {
    // SAFETY: The mapper supplies a unique, writable, permanently mapped virtual range.
    unsafe { GLOBAL_ALLOCATOR.add_range(start, size) };
}

pub(crate) fn initialize_permanent() {
    let start = permanent_heap_start();
    assert_guard_pages(start);

    for offset in (0..PERMANENT_HEAP_SIZE).step_by(usize::try_from(PAGE_SIZE).unwrap()) {
        let address = start.checked_add(u64::try_from(offset).unwrap()).unwrap();
        map_heap_page(address);
    }

    // SAFETY: Every page in this unique virtual range is writable and permanently mapped.
    unsafe { add_permanent_range(start.as_u64().try_into().unwrap(), PERMANENT_HEAP_SIZE) };
}

fn permanent_heap_start() -> VirtualAddress {
    let kernel_end = core::ptr::addr_of!(__kernel_end) as u64;
    let start = crate::address::align_up(kernel_end)
        .unwrap()
        .checked_add(PAGE_SIZE)
        .unwrap();
    VirtualAddress::new(start).unwrap()
}

fn assert_guard_pages(start: VirtualAddress) {
    let lower = VirtualAddress::new(start.as_u64() - PAGE_SIZE).unwrap();
    let upper = start
        .checked_add(u64::try_from(PERMANENT_HEAP_SIZE).unwrap())
        .unwrap();
    assert!(
        !CurrentKernelPageTableBackend::is_mapped(lower),
        "lower heap guard page is mapped"
    );
    assert!(
        !CurrentKernelPageTableBackend::is_mapped(upper),
        "upper heap guard page is mapped"
    );
}

fn map_heap_page(address: VirtualAddress) {
    let frame = frame::allocate().unwrap();
    CurrentKernelPageTableBackend::map_page(address, frame, MappingFlags::WRITABLE);

    let pointer = usize::try_from(address.as_u64()).unwrap() as *mut u8;
    // SAFETY: The mapper created an exclusively owned writable page at this address.
    unsafe { pointer.write_bytes(0, usize::try_from(PAGE_SIZE).unwrap()) };
}

pub(crate) fn statistics() -> (usize, usize, usize, usize) {
    let heap = GLOBAL_ALLOCATOR.heap.lock();
    (
        heap.stats_total_bytes(),
        heap.stats_alloc_user(),
        heap.stats_alloc_actual(),
        GLOBAL_ALLOCATOR.attempts.load(Ordering::Relaxed),
    )
}
