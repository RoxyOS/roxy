use alloc::vec::Vec;

use roxy_memory::{PAGE_SIZE, PhysicalAddress, UserAddress, UserPage};

use super::{AddrSpace, PageState, Permissions, VmError, anonymous::page_count};
use crate::UserRegion;

impl AddrSpace {
    /// Maps a page-aligned physical range into a page-aligned user region.
    ///
    /// The physical pages are caller-owned and remain valid for the kernel lifetime; unmapping or
    /// dropping the address space removes the user mappings without releasing them.
    ///
    /// # Errors
    ///
    /// Returns an error for unaligned or non-representable physical ranges, occupied regions, or
    /// mapping failures.
    pub(super) fn map_physical(
        &mut self,
        region: UserRegion,
        physical_base: PhysicalAddress,
        permissions: Permissions,
    ) -> Result<(), VmError> {
        if !physical_base.as_u64().is_multiple_of(PAGE_SIZE) {
            return Err(VmError::InvalidRange);
        }

        self.ensure_available(region)?;

        for (index, page) in region.pages().enumerate() {
            let address = physical_base
                .checked_add(u64::try_from(index).expect("page count fits u64") * PAGE_SIZE)
                .ok_or(VmError::InvalidRange)?;

            if let Err(error) = self.map_physical_page(page, address, permissions) {
                self.rollback(region, index);

                return Err(error);
            }
        }

        Ok(())
    }

    pub(super) fn map_physical_anywhere(
        &mut self,
        size: usize,
        physical: PhysicalAddress,
        permissions: Permissions,
    ) -> Result<UserAddress, VmError> {
        let region = self.find_free_region(page_count(size)?)?;

        self.map_physical(region, physical, permissions)?;

        Ok(region.start.start_address())
    }

    pub(super) fn map_physical_at(
        &mut self,
        address: UserAddress,
        size: usize,
        physical: PhysicalAddress,
        permissions: Permissions,
    ) -> Result<UserAddress, VmError> {
        let start = UserPage::new(address).ok_or(VmError::InvalidRange)?;
        let region = UserRegion::new(start, page_count(size)?).ok_or(VmError::InvalidRange)?;

        self.map_physical(region, physical, permissions)?;

        Ok(address)
    }

    /// Maps one caller-owned physical page and records its state.
    pub(super) fn map_physical_page(
        &mut self,
        page: UserPage,
        address: PhysicalAddress,
        permissions: Permissions,
    ) -> Result<(), VmError> {
        self.page_table
            .map_user_physical_page(page, address, permissions.into())
            .map_err(super::mapping::mapping_error)?;

        self.pages.insert(
            page,
            PageState::MappedPhysical {
                address,
                permissions,
            },
        );

        Ok(())
    }

    /// Unmaps a page-rounded anonymous allocation or physical mapping.
    ///
    /// Anonymous allocations must match exactly; physical mappings accept any page-aligned
    /// contiguous segment of one mapping.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid ranges, partial anonymous overlaps, or unmapped pages.
    pub(super) fn unmap(&mut self, address: UserAddress, size: usize) -> Result<(), VmError> {
        match self.unmap_anonymous(address, size) {
            Ok(()) => Ok(()),
            Err(VmError::InvalidRange) => self.unmap_physical(address, size),
            Err(error) => Err(error),
        }
    }

    fn unmap_physical(&mut self, address: UserAddress, size: usize) -> Result<(), VmError> {
        let start = UserPage::new(address).ok_or(VmError::InvalidRange)?;
        let region = UserRegion::new(start, page_count(size)?).ok_or(VmError::InvalidRange)?;
        let pages: Vec<UserPage> = region.pages().collect();
        let Some(PageState::MappedPhysical { address: base, .. }) = self.pages.get(&pages[0])
        else {
            return Err(VmError::InvalidRange);
        };

        for (index, page) in pages.iter().enumerate() {
            let expected = base
                .checked_add(u64::try_from(index).expect("page count fits u64") * PAGE_SIZE)
                .ok_or(VmError::InvalidRange)?;
            let matches = match self.pages.get(page) {
                Some(PageState::MappedPhysical { address, .. }) => *address == expected,
                _ => false,
            };

            if !matches {
                return Err(VmError::PartialUnmap);
            }
        }

        for page in pages {
            self.page_table
                .unmap_user_page(page)
                .map_err(super::mapping::mapping_error)?;
            self.pages.remove(&page).ok_or(VmError::MappingFailed)?;
        }

        Ok(())
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::{PhysicalAddress, UserAddress, UserPage, statistics};
    use roxy_test::kernel_test;

    use super::AddrSpace;
    use crate::{AddrSpaceHandle, Permissions, VmError};

    const PHYSICAL_BASE: u64 = 0x4000_0000;
    const PAGE_BYTES: usize = 4096;

    kernel_test!("roxy-vm::physical-lifecycle", physical_lifecycle, {
        let baseline = statistics().allocated_frames;
        let mut space = AddrSpace::new().unwrap();
        let physical = PhysicalAddress::new(PHYSICAL_BASE).unwrap();

        let address = space
            .map_physical_anywhere(2 * PAGE_BYTES, physical, Permissions::ReadWrite)
            .unwrap();
        assert!(space.is_mapped(UserPage::containing(address)));
        assert_eq!(
            space.permissions(UserPage::containing(address)),
            Some(Permissions::ReadWrite)
        );

        space.unmap(address, 2 * PAGE_BYTES).unwrap();
        assert!(!space.is_mapped(UserPage::containing(address)));
        drop(space);
        assert_eq!(statistics().allocated_frames, baseline);
    });

    kernel_test!("roxy-vm::physical-fixed", physical_fixed_placement, {
        let mut space = AddrSpace::new().unwrap();
        let physical = PhysicalAddress::new(PHYSICAL_BASE).unwrap();
        let address = UserAddress::new(0x41_0000).unwrap();

        space
            .map_physical_at(address, PAGE_BYTES, physical, Permissions::ReadWrite)
            .unwrap();
        assert!(space.is_mapped(UserPage::containing(address)));

        space
            .allocate_anonymous_at(address, PAGE_BYTES)
            .unwrap_err();
        space.unmap(address, PAGE_BYTES).unwrap();
        space.allocate_anonymous_at(address, PAGE_BYTES).unwrap();
    });

    kernel_test!(
        "roxy-vm::physical-rejects-invalid",
        physical_rejects_invalid,
        {
            let mut space = AddrSpace::new().unwrap();
            let unaligned = PhysicalAddress::new(PHYSICAL_BASE + 1).unwrap();

            assert_eq!(
                space.map_physical_anywhere(PAGE_BYTES, unaligned, Permissions::ReadWrite),
                Err(VmError::InvalidRange)
            );
            assert_eq!(
                space.map_physical_anywhere(
                    0,
                    PhysicalAddress::new(PHYSICAL_BASE).unwrap(),
                    Permissions::ReadWrite
                ),
                Err(VmError::InvalidRange)
            );
        }
    );

    kernel_test!("roxy-vm::physical-unmap-segment", physical_unmap_segment, {
        let mut space = AddrSpace::new().unwrap();
        let physical = PhysicalAddress::new(PHYSICAL_BASE).unwrap();
        let address = space
            .map_physical_anywhere(4 * PAGE_BYTES, physical, Permissions::ReadWrite)
            .unwrap();
        let segment = UserAddress::new(address.as_u64() + PAGE_BYTES as u64).unwrap();

        space.unmap(segment, 2 * PAGE_BYTES).unwrap();
        assert!(!space.is_mapped(UserPage::containing(segment)));
        space.unmap(address, 4 * PAGE_BYTES).unwrap_err();
        space.unmap(address, PAGE_BYTES).unwrap();
        space
            .unmap(
                UserAddress::new(address.as_u64() + 3 * PAGE_BYTES as u64).unwrap(),
                PAGE_BYTES,
            )
            .unwrap();
    });

    kernel_test!("roxy-vm::physical-fork-shares", physical_fork_shares, {
        let space = AddrSpace::new().unwrap().into_handle();
        let physical = PhysicalAddress::new(PHYSICAL_BASE).unwrap();
        let address = space
            .map_physical(PAGE_BYTES, physical, Permissions::ReadWrite)
            .unwrap();

        let copy = space.fork_copy().unwrap();
        // The copy reuses the same physical frame instead of allocating a private one.
        assert_eq!(physical_address_of(&copy, address), Some(PHYSICAL_BASE));
    });

    fn physical_address_of(space: &AddrSpaceHandle, address: UserAddress) -> Option<u64> {
        let pages = &space.0.lock().pages;
        let state = pages.get(&UserPage::containing(address))?;

        match state {
            super::PageState::MappedPhysical { address, .. } => Some(address.as_u64()),
            _ => None,
        }
    }
}
