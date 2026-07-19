use core::num::NonZeroUsize;

use roxy_memory::{PAGE_SIZE, UserAddress, UserPage};

use super::{AddrSpace, AnonymousAllocation, Permissions, VmError};
use crate::UserRegion;

const ANONYMOUS_START: u64 = 0x0000_4000_0000_0000;
const ANONYMOUS_END: u64 = 0x0000_7fff_fffe_e000;

impl AddrSpace {
    pub(super) fn allocate_anonymous(&mut self, size: usize) -> Result<UserAddress, VmError> {
        let page_count = page_count(size)?;
        let region = self.find_anonymous_region(page_count)?;

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

    fn find_anonymous_region(&self, page_count: NonZeroUsize) -> Result<UserRegion, VmError> {
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

fn page_count(size: usize) -> Result<NonZeroUsize, VmError> {
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

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::{UserPage, statistics};
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

    kernel_test!("roxy-vm::anonymous-rejects-invalid", anonymous_rejects_invalid, {
        let mut space = AddrSpace::new().unwrap();
        assert_eq!(space.allocate_anonymous(0), Err(VmError::InvalidRange));
        assert_eq!(space.allocate_anonymous(usize::MAX), Err(VmError::InvalidRange));
    });
}
