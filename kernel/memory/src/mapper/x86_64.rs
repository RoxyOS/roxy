use spin::{Mutex, Once};
use x86_64::{
    VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper as PageMapper, OffsetPageTable, Page, PageTable, PageTableFlags,
        PhysFrame, Size4KiB,
    },
};

use crate::{
    OwnedFrame, VirtualAddress, frame,
    tlb::{self, TlbInvalidation},
};

use super::{Mapper, MappingFlags, sealed};

static MAPPER: Once<Mutex<X86_64Mapper>> = Once::new();

pub(crate) struct X86_64Mapper {
    mapper: OffsetPageTable<'static>,
}

impl sealed::Sealed for X86_64Mapper {}

impl Mapper for X86_64Mapper {
    fn initialize(hhdm_offset: u64) {
        assert!(!MAPPER.is_completed(), "kernel mapper initialized twice");
        MAPPER.call_once(|| Mutex::new(Self::from_active_table(hhdm_offset)));
    }

    fn is_mapped(address: VirtualAddress) -> bool {
        let mapper = MAPPER.get().unwrap().lock();
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(address.as_u64()));
        mapper.mapper.translate_page(page).is_ok()
    }

    fn map_page(address: VirtualAddress, frame: OwnedFrame, flags: MappingFlags) {
        MAPPER.get().unwrap().lock().map_page(address, frame, flags);
    }
}

impl X86_64Mapper {
    fn from_active_table(hhdm_offset: u64) -> Self {
        let (level_4_frame, _) = Cr3::read();
        let table_address = hhdm_offset
            .checked_add(level_4_frame.start_address().as_u64())
            .unwrap();
        let table_pointer = usize::try_from(table_address).unwrap() as *mut PageTable;

        // SAFETY: Limine maps all physical memory at hhdm_offset, and CR3 names the active PML4.
        let page_table = unsafe { &mut *table_pointer };
        // SAFETY: page_table is the uniquely borrowed active PML4 and the HHDM offset is valid.
        let mapper = unsafe { OffsetPageTable::new(page_table, VirtAddr::new(hhdm_offset)) };
        Self { mapper }
    }

    fn map_page(&mut self, address: VirtualAddress, frame: OwnedFrame, flags: MappingFlags) {
        let page = Page::<Size4KiB>::containing_address(VirtAddr::new(address.as_u64()));
        assert!(
            self.mapper.translate_page(page).is_err(),
            "virtual page is already mapped"
        );

        let physical_address = frame.start_address();
        let physical_frame =
            PhysFrame::containing_address(::x86_64::PhysAddr::new(physical_address.as_u64()));
        let flags = page_table_flags(flags);

        // SAFETY: The page is unmapped and the caller transfers unique ownership of the frame.
        let flush = unsafe {
            self.mapper
                .map_to(page, physical_frame, flags, &mut PageTableAllocator)
        }
        .unwrap();
        frame.transfer_to_mapping();
        flush.ignore();
        tlb::invalidate(TlbInvalidation::Page(address));
    }
}

fn page_table_flags(flags: MappingFlags) -> PageTableFlags {
    let mut page_flags = PageTableFlags::PRESENT;
    page_flags.set(
        PageTableFlags::WRITABLE,
        flags.contains(MappingFlags::WRITABLE),
    );
    page_flags.set(
        PageTableFlags::NO_EXECUTE,
        !flags.contains(MappingFlags::EXECUTABLE),
    );
    page_flags.set(
        PageTableFlags::USER_ACCESSIBLE,
        flags.contains(MappingFlags::USER),
    );
    page_flags
}

struct PageTableAllocator;

// SAFETY: Every returned frame is uniquely allocated and suitable for a 4 KiB page table.
unsafe impl FrameAllocator<Size4KiB> for PageTableAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let frame = frame::allocate()?;
        let address = frame.start_address();
        frame.transfer_to_mapping();
        Some(PhysFrame::containing_address(::x86_64::PhysAddr::new(
            address.as_u64(),
        )))
    }
}
