use alloc::vec::Vec;

use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB, Translate,
        mapper::{FlagUpdateError, MapToError, TranslateResult, UnmapError},
    },
};

use crate::{
    OwnedFrame, PageRef, PhysicalAddress, UserPage, frame,
    frame::{hhdm_offset, physical_pointer},
    mapper::{
        MappingError, PagePermissions, PageTableToken,
        pagetable::{AddrSpacePageTableBackend, sealed},
    },
};

use super::kernel::copy_kernel_entries;
#[cfg(feature = "kernel-test")]
use super::kernel::kernel_entries_match;

pub(crate) struct X86_64AddrSpacePageTable {
    mapper: OffsetPageTable<'static>,
    root: OwnedFrame,
    table_frames: Vec<OwnedFrame>,
}

impl X86_64AddrSpacePageTable {
    #[cfg(feature = "kernel-test")]
    pub(crate) fn has_empty_user_half(&self) -> bool {
        self.mapper
            .level_4_table()
            .iter()
            .take(256)
            .all(PageTableEntry::is_unused)
    }

    #[cfg(feature = "kernel-test")]
    pub(crate) fn has_current_kernel_half(&self) -> bool {
        kernel_entries_match(self.mapper.level_4_table())
    }

    #[cfg(feature = "kernel-test")]
    pub(crate) fn is_active(&self) -> bool {
        Cr3::read().0.start_address().as_u64() == self.root_address().as_u64()
    }
}

impl sealed::Sealed for X86_64AddrSpacePageTable {}

impl AddrSpacePageTableBackend for X86_64AddrSpacePageTable {
    fn new() -> Result<Self, MappingError> {
        let root = frame::allocate().ok_or(MappingError::OutOfMemory)?;
        root.zero();
        let root_table = table_at(root.start_address());
        copy_kernel_entries(root_table);

        // SAFETY: root_table is a valid, uniquely owned PML4 and hhdm_offset maps all frames.
        let mapper = unsafe { OffsetPageTable::new(root_table, VirtAddr::new(hhdm_offset())) };
        Ok(Self {
            mapper,
            root,
            table_frames: Vec::new(),
        })
    }

    fn root_address(&self) -> PhysicalAddress {
        self.root.start_address()
    }

    unsafe fn activate(&self) -> PageTableToken {
        let (previous, flags) = Cr3::read();
        let root = PhysFrame::containing_address(PhysAddr::new(self.root_address().as_u64()));
        // SAFETY: The caller keeps the complete hierarchy rooted at `root` alive while active.
        unsafe { Cr3::write(root, flags) };
        PageTableToken {
            root: PhysicalAddress::new(previous.start_address().as_u64()).unwrap(),
            flags: flags.bits(),
        }
    }

    unsafe fn restore(token: PageTableToken) {
        let root = PhysFrame::containing_address(PhysAddr::new(token.root.as_u64()));
        let flags = Cr3Flags::from_bits_truncate(token.flags);
        // SAFETY: The caller guarantees the saved hierarchy remains alive.
        unsafe { Cr3::write(root, flags) };
    }

    fn map_user_page(
        &mut self,
        page: UserPage,
        frame: &PageRef,
        permissions: PagePermissions,
    ) -> Result<(), MappingError> {
        let page = page_from(page);
        let physical = PhysFrame::containing_address(PhysAddr::new(frame.start_address().as_u64()));
        let mut allocator = TableFrameAllocator {
            frames: &mut self.table_frames,
        };

        // SAFETY: the leaf frame remains owned by PageRef and flags enforce user W^X mappings.
        let flush = unsafe {
            self.mapper
                .map_to(page, physical, user_page_flags(permissions), &mut allocator)
        }
        .map_err(|error| map_error(&error))?;
        flush.ignore();
        Ok(())
    }

    fn unmap_user_page(&mut self, page: UserPage) -> Result<(), MappingError> {
        let (_, flush) = self
            .mapper
            .unmap(page_from(page))
            .map_err(|error| unmap_error(&error))?;
        flush.ignore();
        Ok(())
    }

    fn protect_user_page(
        &mut self,
        page: UserPage,
        permissions: PagePermissions,
    ) -> Result<(), MappingError> {
        // SAFETY: permissions only select user W^X access for this owned page table.
        let flush = unsafe {
            self.mapper
                .update_flags(page_from(page), user_page_flags(permissions))
        }
        .map_err(|error| flag_error(&error))?;
        flush.ignore();
        Ok(())
    }

    fn is_user_page_mapped(&self, page: UserPage) -> bool {
        self.mapper.translate_page(page_from(page)).is_ok()
    }

    fn user_page_permissions(&self, page: UserPage) -> Result<PagePermissions, MappingError> {
        let address = VirtAddr::new(page.start_address().as_u64());
        let TranslateResult::Mapped { flags, .. } = self.mapper.translate(address) else {
            return Err(MappingError::NotMapped);
        };

        decode_user_permissions(flags)
    }
}

#[cfg(feature = "kernel-test")]
use x86_64::structures::paging::page_table::PageTableEntry;

fn table_at(address: PhysicalAddress) -> &'static mut PageTable {
    let pointer = physical_pointer::<PageTable>(address);
    // SAFETY: the caller provides a page-aligned live frame mapped through the HHDM.
    unsafe { &mut *pointer }
}

fn page_from(page: UserPage) -> Page<Size4KiB> {
    Page::from_start_address(VirtAddr::new(page.start_address().as_u64())).unwrap()
}

fn user_page_flags(permissions: PagePermissions) -> PageTableFlags {
    let mut flags = PageTableFlags::PRESENT | PageTableFlags::USER_ACCESSIBLE;
    flags.set(
        PageTableFlags::WRITABLE,
        permissions == PagePermissions::ReadWrite,
    );
    flags.set(
        PageTableFlags::NO_EXECUTE,
        permissions != PagePermissions::ReadExecute,
    );
    flags
}

fn decode_user_permissions(flags: PageTableFlags) -> Result<PagePermissions, MappingError> {
    if !flags.contains(PageTableFlags::USER_ACCESSIBLE)
        || flags.contains(PageTableFlags::WRITABLE) && !flags.contains(PageTableFlags::NO_EXECUTE)
    {
        return Err(MappingError::InvalidHierarchy);
    }

    if flags.contains(PageTableFlags::WRITABLE) {
        Ok(PagePermissions::ReadWrite)
    } else if flags.contains(PageTableFlags::NO_EXECUTE) {
        Ok(PagePermissions::ReadOnly)
    } else {
        Ok(PagePermissions::ReadExecute)
    }
}

fn map_error(error: &MapToError<Size4KiB>) -> MappingError {
    match error {
        MapToError::FrameAllocationFailed => MappingError::OutOfMemory,
        MapToError::PageAlreadyMapped(_) => MappingError::AlreadyMapped,
        MapToError::ParentEntryHugePage => MappingError::InvalidHierarchy,
    }
}

fn unmap_error(error: &UnmapError) -> MappingError {
    match error {
        UnmapError::PageNotMapped => MappingError::NotMapped,
        UnmapError::ParentEntryHugePage | UnmapError::InvalidFrameAddress(_) => {
            MappingError::InvalidHierarchy
        }
    }
}

fn flag_error(error: &FlagUpdateError) -> MappingError {
    match error {
        FlagUpdateError::PageNotMapped => MappingError::NotMapped,
        FlagUpdateError::ParentEntryHugePage => MappingError::InvalidHierarchy,
    }
}

struct TableFrameAllocator<'a> {
    frames: &'a mut Vec<OwnedFrame>,
}

// SAFETY: every returned frame is zeroed, uniquely owned, and retained by the page table.
unsafe impl FrameAllocator<Size4KiB> for TableFrameAllocator<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.frames.try_reserve(1).ok()?;
        let frame = frame::allocate()?;
        frame.zero();
        let physical = frame.start_address();
        self.frames.push(frame);
        Some(PhysFrame::containing_address(PhysAddr::new(
            physical.as_u64(),
        )))
    }
}
