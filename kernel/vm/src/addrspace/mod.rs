mod io;
mod mapping;
mod stack;
mod types;

use alloc::collections::BTreeMap;

use roxy_memory::{AddrSpacePageTable, PageRef, PageTableToken, PhysicalAddress, UserPage};

pub use types::{Permissions, VmError};

pub struct AddrSpace {
    pub(super) pages: BTreeMap<UserPage, PageState>,
    page_table: AddrSpacePageTable,
}

impl AddrSpace {
    /// Creates an empty address space.
    ///
    /// # Errors
    ///
    /// Returns an error when its root page table cannot be allocated.
    pub fn new() -> Result<Self, VmError> {
        Ok(Self {
            pages: BTreeMap::new(),
            page_table: AddrSpacePageTable::new().map_err(|_| VmError::OutOfMemory)?,
        })
    }

    #[must_use]
    pub fn root_address(&self) -> PhysicalAddress {
        self.page_table.root_address()
    }

    #[must_use]
    pub fn is_mapped(&self, page: UserPage) -> bool {
        matches!(self.pages.get(&page), Some(PageState::Mapped { .. }))
    }

    /// Activates this address space until the returned guard is dropped.
    #[must_use]
    pub fn activate(&self) -> AddrSpaceGuard<'_> {
        // SAFETY: The guard borrows this address space and restores the previous table in Drop.
        let previous = unsafe { self.page_table.activate() };
        AddrSpaceGuard {
            _addrspace: self,
            previous: Some(previous),
        }
    }

    #[must_use]
    pub fn permissions(&self, page: UserPage) -> Option<Permissions> {
        let PageState::Mapped { permissions, .. } = self.pages.get(&page)? else {
            return None;
        };
        Some(*permissions)
    }
}

/// Restores the previously active page table when it leaves scope.
pub struct AddrSpaceGuard<'a> {
    _addrspace: &'a AddrSpace,
    previous: Option<PageTableToken>,
}

impl Drop for AddrSpaceGuard<'_> {
    fn drop(&mut self) {
        let previous = self.previous.take().unwrap();
        // SAFETY: The previously active hierarchy outlives this nested activation scope.
        unsafe { AddrSpacePageTable::restore(previous) };
    }
}

impl Drop for AddrSpace {
    fn drop(&mut self) {
        for (page, state) in &self.pages {
            if matches!(state, PageState::Mapped { .. }) {
                self.page_table.unmap_user_page(*page).unwrap();
            }
        }
    }
}

pub(super) enum PageState {
    Mapped {
        frame: PageRef,
        permissions: Permissions,
    },
    Guard,
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::num::NonZeroUsize;

    use roxy_memory::{UserAddress, UserPage, statistics};
    use roxy_test::kernel_test;

    use super::{AddrSpace, Permissions};
    use crate::UserRegion;

    kernel_test!("roxy-vm::addrspace-teardown", addrspace_teardown, {
        let baseline = statistics().allocated_frames;
        {
            let mut space = AddrSpace::new().unwrap();
            space
                .map_zeroed(region_at(0x60_0000, 8), Permissions::ReadWrite)
                .unwrap();
            space.map_stack().unwrap();
            assert!(statistics().allocated_frames > baseline);
        }
        assert_eq!(statistics().allocated_frames, baseline);
    });

    fn region_at(address: u64, pages: usize) -> UserRegion {
        let address = UserAddress::new(address).unwrap();
        let page = UserPage::new(address).unwrap();
        UserRegion::new(page, NonZeroUsize::new(pages).unwrap()).unwrap()
    }
}
