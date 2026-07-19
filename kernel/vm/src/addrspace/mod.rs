mod anonymous;
mod io;
mod mapping;
mod stack;
mod types;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};

use roxy_memory::{AddrSpacePageTable, PageRef, PageTableToken, PhysicalAddress, UserPage};
use roxy_utils::Lock;

pub use types::{Permissions, VmError};

pub struct AddrSpace {
    pub(super) pages: BTreeMap<UserPage, PageState>,
    pub(super) anonymous: BTreeMap<UserPage, AnonymousAllocation>,
    page_table: AddrSpacePageTable,
    id: AddrSpaceId,
}

#[derive(Clone)]
pub struct AddrSpaceHandle(Arc<Lock<AddrSpace>>);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AddrSpaceId(u64);

#[derive(Clone, Copy)]
pub(super) struct AnonymousAllocation {
    pub(super) region: crate::UserRegion,
    pub(super) requested_size: usize,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

impl AddrSpace {
    /// Creates an empty address space.
    ///
    /// # Errors
    ///
    /// Returns an error when its root page table cannot be allocated.
    pub fn new() -> Result<Self, VmError> {
        Ok(Self {
            pages: BTreeMap::new(),
            anonymous: BTreeMap::new(),
            page_table: AddrSpacePageTable::new().map_err(|_| VmError::OutOfMemory)?,
            id: AddrSpaceId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
        })
    }

    #[must_use]
    pub fn into_handle(self) -> AddrSpaceHandle {
        AddrSpaceHandle(Arc::new(Lock::new(self)))
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

impl AddrSpaceHandle {
    #[must_use]
    pub fn id(&self) -> AddrSpaceId {
        self.0.lock().id
    }

    #[must_use]
    pub fn root_address(&self) -> PhysicalAddress {
        self.0.lock().root_address()
    }

    /// Creates a private, zero-filled, writable anonymous allocation.
    ///
    /// # Errors
    ///
    /// Returns an error for zero or overflowing sizes, address-space exhaustion, or allocation
    /// failure.
    pub fn allocate_anonymous(&self, size: usize) -> Result<roxy_memory::UserAddress, VmError> {
        self.0.lock().allocate_anonymous(size)
    }

    /// Releases one complete anonymous allocation.
    ///
    /// # Errors
    ///
    /// Returns an error unless `address` and `size` exactly identify a live allocation.
    pub fn free_anonymous(
        &self,
        address: roxy_memory::UserAddress,
        size: usize,
    ) -> Result<(), VmError> {
        self.0.lock().free_anonymous(address, size)
    }

    /// Unmaps one complete page-rounded anonymous allocation.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid ranges or requests that only partially overlap an allocation.
    pub fn unmap_anonymous(
        &self,
        address: roxy_memory::UserAddress,
        size: usize,
    ) -> Result<(), VmError> {
        self.0.lock().unmap_anonymous(address, size)
    }

    /// Makes this address space active until another page table is selected.
    pub fn activate(&self) {
        // SAFETY: this strong handle keeps the complete page-table hierarchy alive while selected.
        let _ = unsafe { self.0.lock().page_table.activate() };
    }

    /// Reads a validated mapped range through this shared handle.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, unmapped, or inaccessible ranges.
    pub fn read_bytes(
        &self,
        address: roxy_memory::UserAddress,
        output: &mut [u8],
    ) -> Result<(), VmError> {
        self.0.lock().read_bytes(address, output)
    }

    /// Writes a validated writable range through this shared handle.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid, unmapped, or read-only ranges.
    pub fn write_bytes(
        &self,
        address: roxy_memory::UserAddress,
        input: &[u8],
    ) -> Result<(), VmError> {
        self.0.lock().write_bytes(address, input)
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

    kernel_test!(
        "roxy-vm::addrspace-handle-lifetime",
        addrspace_handle_lifetime,
        {
            let baseline = statistics().allocated_frames;
            let handle = AddrSpace::new().unwrap().into_handle();
            let clone = handle.clone();
            assert!(statistics().allocated_frames > baseline);

            drop(handle);
            assert!(statistics().allocated_frames > baseline);

            drop(clone);
            assert_eq!(statistics().allocated_frames, baseline);
        }
    );

    fn region_at(address: u64, pages: usize) -> UserRegion {
        let address = UserAddress::new(address).unwrap();
        let page = UserPage::new(address).unwrap();
        UserRegion::new(page, NonZeroUsize::new(pages).unwrap()).unwrap()
    }
}
