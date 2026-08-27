use core::num::NonZeroUsize;

use roxy_memory::{PAGE_SIZE, UserAddress, UserPage};

use super::{AddrSpace, AnonymousAllocation, Permissions, VmError};
use crate::UserRegion;

const ANONYMOUS_START: u64 = 0x0000_4000_0000_0000;
const ANONYMOUS_END: u64 = 0x0000_7fff_fffe_e000;

impl AddrSpace {
    pub(super) fn allocate_anonymous(&mut self, size: usize) -> Result<UserAddress, VmError> {
        let page_count = page_count(size)?;
        let region = self.find_free_region(page_count)?;

        self.map_zeroed(region, Permissions::ReadWrite)?;
        self.anonymous.insert(
            region.start,
            AnonymousAllocation {
                region,
                requested_size: size,
            },
        );

        Ok(region.start.start_address())
    }

    pub(super) fn allocate_anonymous_at(
        &mut self,
        address: UserAddress,
        size: usize,
    ) -> Result<UserAddress, VmError> {
        let start = UserPage::new(address).ok_or(VmError::InvalidRange)?;
        let region = UserRegion::new(start, page_count(size)?).ok_or(VmError::InvalidRange)?;

        self.map_zeroed(region, Permissions::ReadWrite)?;
        self.anonymous.insert(
            start,
            AnonymousAllocation {
                region,
                requested_size: size,
            },
        );

        Ok(address)
    }

    pub(super) fn free_anonymous(
        &mut self,
        address: UserAddress,
        size: usize,
    ) -> Result<(), VmError> {
        let start = UserPage::new(address).ok_or(VmError::InvalidRange)?;
        let allocation = self
            .anonymous
            .get(&start)
            .copied()
            .filter(|allocation| allocation.requested_size == size)
            .ok_or(VmError::InvalidRange)?;

        self.release_anonymous(start, allocation)
    }

    pub(super) fn unmap_anonymous(
        &mut self,
        address: UserAddress,
        size: usize,
    ) -> Result<(), VmError> {
        let start = UserPage::new(address).ok_or(VmError::InvalidRange)?;
        let requested = UserRegion::new(start, page_count(size)?).ok_or(VmError::InvalidRange)?;
        let allocation = self
            .anonymous
            .values()
            .copied()
            .find(|allocation| regions_overlap(requested, allocation.region))
            .ok_or(VmError::InvalidRange)?;

        if requested.start != allocation.region.start
            || requested.page_count != allocation.region.page_count
        {
            return Err(VmError::PartialUnmap);
        }

        self.release_anonymous(start, allocation)
    }

    fn release_anonymous(
        &mut self,
        start: UserPage,
        allocation: AnonymousAllocation,
    ) -> Result<(), VmError> {
        if !allocation.region.pages().all(|page| {
            matches!(self.pages.get(&page), Some(super::PageState::Mapped { .. }))
                && self.page_table.is_user_page_mapped(page)
        }) {
            return Err(VmError::MappingFailed);
        }

        for page in allocation.region.pages() {
            self.page_table
                .unmap_user_page(page)
                .map_err(super::mapping::mapping_error)?;
            self.pages.remove(&page).ok_or(VmError::MappingFailed)?;
        }

        self.anonymous.remove(&start);

        Ok(())
    }

    pub(super) fn find_free_region(&self, page_count: NonZeroUsize) -> Result<UserRegion, VmError> {
        let bytes = u64::try_from(page_count.get())
            .ok()
            .and_then(|pages| pages.checked_mul(PAGE_SIZE))
            .ok_or(VmError::InvalidRange)?;
        let mut end = ANONYMOUS_END;

        for page in self.pages.keys().rev() {
            let address = page.start_address().as_u64();

            if address >= end || address < ANONYMOUS_START {
                continue;
            }

            if let Some(start) = end.checked_sub(bytes)
                && start > address
            {
                return region_at(start, page_count);
            }
            end = address;
        }

        let start = end.checked_sub(bytes).ok_or(VmError::OutOfMemory)?;

        if start < ANONYMOUS_START {
            return Err(VmError::OutOfMemory);
        }

        region_at(start, page_count)
    }
}

pub(super) fn page_count(size: usize) -> Result<NonZeroUsize, VmError> {
    let page_size = usize::try_from(PAGE_SIZE).unwrap();
    size.checked_add(page_size - 1)
        .map(|bytes| bytes / page_size)
        .and_then(NonZeroUsize::new)
        .ok_or(VmError::InvalidRange)
}

fn region_at(start: u64, page_count: NonZeroUsize) -> Result<UserRegion, VmError> {
    let start = UserAddress::new(start)
        .and_then(UserPage::new)
        .ok_or(VmError::InvalidRange)?;
    UserRegion::new(start, page_count).ok_or(VmError::InvalidRange)
}

fn regions_overlap(left: UserRegion, right: UserRegion) -> bool {
    let left_end = left.start.checked_add(left.page_count.get() - 1).unwrap();
    let right_end = right.start.checked_add(right.page_count.get() - 1).unwrap();

    left.start <= right_end && right.start <= left_end
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::{PAGE_SIZE, UserAddress, UserPage, statistics};
    use roxy_test::kernel_test;

    use super::AddrSpace;
    use crate::VmError;

    kernel_test!("roxy-vm::anonymous-lifecycle", anonymous_lifecycle, {
        let baseline = statistics().allocated_frames;
        let mut space = AddrSpace::new().unwrap();
        let first = space.allocate_anonymous(1).unwrap();
        let second = space.allocate_anonymous(4097).unwrap();
        assert!(second < first);
        assert_eq!(space.read_bytes(first, &mut [0xff; 1]), Ok(()));
        assert_eq!(space.write_bytes(first, &[0x5a]), Ok(()));
        assert!(space.is_mapped(UserPage::containing(first)));

        assert_eq!(space.free_anonymous(first, 2), Err(VmError::InvalidRange));
        space.free_anonymous(first, 1).unwrap();
        assert!(!space.is_mapped(UserPage::containing(first)));
        assert_eq!(space.free_anonymous(first, 1), Err(VmError::InvalidRange));
        let reused = space.allocate_anonymous(1).unwrap();
        assert_eq!(reused, first);

        space.free_anonymous(reused, 1).unwrap();
        space.free_anonymous(second, 4097).unwrap();
        drop(space);
        assert_eq!(statistics().allocated_frames, baseline);
    });

    kernel_test!(
        "roxy-vm::anonymous-rejects-invalid",
        anonymous_rejects_invalid,
        {
            let mut space = AddrSpace::new().unwrap();
            assert_eq!(space.allocate_anonymous(0), Err(VmError::InvalidRange));
            assert_eq!(
                space.allocate_anonymous(usize::MAX),
                Err(VmError::InvalidRange)
            );
        }
    );

    kernel_test!("roxy-vm::anonymous-unmap", anonymous_unmap, {
        let baseline = statistics().allocated_frames;
        let mut space = AddrSpace::new().unwrap();
        let rounded = space.allocate_anonymous(4097).unwrap();
        let allocation = space.allocate_anonymous(8192).unwrap();
        let interior = UserAddress::new(allocation.as_u64() + PAGE_SIZE).unwrap();
        let unaligned = UserAddress::new(allocation.as_u64() + 1).unwrap();

        space.unmap_anonymous(rounded, 8192).unwrap();
        assert_eq!(
            space.unmap_anonymous(allocation, 4096),
            Err(VmError::PartialUnmap)
        );
        assert_eq!(
            space.unmap_anonymous(interior, 4096),
            Err(VmError::PartialUnmap)
        );
        assert_eq!(
            space.unmap_anonymous(unaligned, 8192),
            Err(VmError::InvalidRange)
        );

        space.unmap_anonymous(allocation, 8192).unwrap();
        drop(space);
        assert_eq!(statistics().allocated_frames, baseline);
    });
}
