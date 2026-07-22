use crate::{PageRef, PhysicalAddress, UserPage};
use spin::Once;

#[cfg(target_arch = "x86_64")]
use super::x86_64::X86_64AddrSpacePageTable;

#[cfg(target_arch = "x86_64")]
type CurrentAddrSpacePageTableBackend = X86_64AddrSpacePageTable;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PagePermissions {
    ReadOnly,
    ReadWrite,
    ReadExecute,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MappingError {
    OutOfMemory,
    AlreadyMapped,
    NotMapped,
    InvalidHierarchy,
}

/// Owns the page-table hierarchy for one independently managed address space.
///
/// Unlike the active kernel mapper, this value is created inactive and only changes the current
/// CR3 within an explicit activation scope. It owns a separate PML4 plus all private lower-level
/// table frames, copies the shared kernel half at creation, and only exposes lower-half user
/// mappings. Leaf data frames remain owned separately through [`PageRef`]. Dropping this value
/// reclaims its private page-table frames; it is therefore a lifecycle owner, not a raw container.
pub struct AddrSpacePageTable(CurrentAddrSpacePageTableBackend);

/// Opaque state required to restore a previously active page table.
#[derive(Clone, Copy)]
pub struct PageTableToken {
    pub(crate) root: PhysicalAddress,
    pub(crate) flags: u64,
}

static KERNEL_PAGE_TABLE: Once<PageTableToken> = Once::new();

pub(crate) fn initialize_kernel_page_table() {
    KERNEL_PAGE_TABLE.call_once(CurrentAddrSpacePageTableBackend::current);
}

/// Selects the boot-owned kernel page table.
///
/// # Panics
///
/// Panics when memory initialization has not completed.
pub fn activate_kernel_page_table() {
    let token = *KERNEL_PAGE_TABLE.get().expect("memory not initialized");
    // SAFETY: the boot-owned kernel page table remains alive for the entire kernel lifetime.
    unsafe { AddrSpacePageTable::restore(token) };
}

#[must_use]
/// Returns the boot-owned kernel page-table root.
///
/// # Panics
///
/// Panics when memory initialization has not completed.
pub fn kernel_page_table_root() -> PhysicalAddress {
    KERNEL_PAGE_TABLE
        .get()
        .expect("memory not initialized")
        .root
}

impl AddrSpacePageTable {
    /// Creates an empty address-space page table with the current kernel mappings.
    ///
    /// # Errors
    ///
    /// Returns an error when the root frame cannot be allocated.
    pub fn new() -> Result<Self, MappingError> {
        CurrentAddrSpacePageTableBackend::new().map(Self)
    }

    #[must_use]
    pub fn root_address(&self) -> PhysicalAddress {
        self.0.root_address()
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        CurrentAddrSpacePageTableBackend::current().root == self.root_address()
    }

    /// Installs this address space and returns the previously active page table.
    ///
    /// # Safety
    ///
    /// The caller must keep this page table alive until the returned token has been restored.
    #[must_use]
    pub unsafe fn activate(&self) -> PageTableToken {
        // SAFETY: The caller guarantees that this page table remains alive while active.
        unsafe { self.0.activate() }
    }

    /// Restores a page table returned by [`Self::activate`].
    ///
    /// # Safety
    ///
    /// The page-table hierarchy represented by `token` must still be alive.
    pub unsafe fn restore(token: PageTableToken) {
        // SAFETY: The caller guarantees that the saved page table remains alive.
        unsafe { CurrentAddrSpacePageTableBackend::restore(token) };
    }

    /// Maps a user page to an owned frame reference.
    ///
    /// # Errors
    ///
    /// Returns an error on allocation failure, duplicate mapping, or an invalid hierarchy.
    pub fn map_user_page(
        &mut self,
        page: UserPage,
        frame: &PageRef,
        permissions: PagePermissions,
    ) -> Result<(), MappingError> {
        self.0.map_user_page(page, frame, permissions)
    }

    /// Removes an existing user page mapping.
    ///
    /// # Errors
    ///
    /// Returns an error when the page is not mapped or the hierarchy is invalid.
    pub fn unmap_user_page(&mut self, page: UserPage) -> Result<(), MappingError> {
        self.0.unmap_user_page(page, self.is_active())
    }

    /// Changes the permissions of an existing user page.
    ///
    /// # Errors
    ///
    /// Returns an error when the page is not mapped or the hierarchy is invalid.
    pub fn protect_user_page(
        &mut self,
        page: UserPage,
        permissions: PagePermissions,
    ) -> Result<(), MappingError> {
        let flush_active = self.is_active();

        self.0.protect_user_page(page, permissions, flush_active)
    }

    #[must_use]
    pub fn is_user_page_mapped(&self, page: UserPage) -> bool {
        self.0.is_user_page_mapped(page)
    }

    /// Returns the effective leaf permissions of a mapped user page.
    ///
    /// # Errors
    ///
    /// Returns an error for an unmapped page or an invalid user mapping.
    pub fn user_page_permissions(&self, page: UserPage) -> Result<PagePermissions, MappingError> {
        self.0.user_page_permissions(page)
    }
}

pub(crate) trait AddrSpacePageTableBackend: sealed::Sealed {
    fn new() -> Result<Self, MappingError>
    where
        Self: Sized;

    fn root_address(&self) -> PhysicalAddress;

    fn current() -> PageTableToken;

    unsafe fn activate(&self) -> PageTableToken;

    unsafe fn restore(token: PageTableToken);

    fn map_user_page(
        &mut self,
        page: UserPage,
        frame: &PageRef,
        permissions: PagePermissions,
    ) -> Result<(), MappingError>;

    fn unmap_user_page(&mut self, page: UserPage, flush: bool) -> Result<(), MappingError>;

    fn protect_user_page(
        &mut self,
        page: UserPage,
        permissions: PagePermissions,
        flush_active: bool,
    ) -> Result<(), MappingError>;

    fn is_user_page_mapped(&self, page: UserPage) -> bool;

    fn user_page_permissions(&self, page: UserPage) -> Result<PagePermissions, MappingError>;
}

pub(crate) mod sealed {
    pub trait Sealed {}
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{AddrSpacePageTable, MappingError, PagePermissions};
    use crate::{UserAddress, UserPage, frame, statistics};

    kernel_test!("roxy-memory::addrspace-page-table", addrspace_page_table, {
        let baseline = statistics().allocated_frames;

        {
            let mut table = AddrSpacePageTable::new().unwrap();
            assert!(table.0.has_empty_user_half());
            assert!(table.0.has_current_kernel_half());
            let page = UserPage::new(UserAddress::new(0x20_0000).unwrap()).unwrap();
            let frame = frame::allocate_zeroed().unwrap();

            table
                .map_user_page(page, &frame, PagePermissions::ReadWrite)
                .unwrap();
            assert!(table.is_user_page_mapped(page));
            assert_eq!(
                table.map_user_page(page, &frame, PagePermissions::ReadOnly),
                Err(MappingError::AlreadyMapped)
            );
            table
                .protect_user_page(page, PagePermissions::ReadExecute)
                .unwrap();
            assert_eq!(
                table.user_page_permissions(page),
                Ok(PagePermissions::ReadExecute)
            );
            table.unmap_user_page(page).unwrap();
            assert!(!table.is_user_page_mapped(page));
        }

        assert_eq!(statistics().allocated_frames, baseline);
    });

    kernel_test!("roxy-memory::addrspace-activation", addrspace_activation, {
        let table = AddrSpacePageTable::new().unwrap();
        assert!(!table.0.is_active());
        // SAFETY: the table remains alive until the previous CR3 is restored below.
        let previous = unsafe { table.activate() };
        assert!(table.0.is_active());
        // SAFETY: the original kernel page table remains alive for the entire boot.
        unsafe { AddrSpacePageTable::restore(previous) };
        assert!(!table.0.is_active());
    });
}
