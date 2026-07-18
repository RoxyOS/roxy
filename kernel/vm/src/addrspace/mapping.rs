use roxy_memory::{MappingError, PagePermissions, UserPage, frame};

use super::{AddrSpace, PageState, Permissions, VmError};
use crate::UserRegion;

impl AddrSpace {
    /// Maps a zero-filled user region.
    ///
    /// # Errors
    ///
    /// Returns an error on overlap, invalid ranges, or allocation failure.
    pub fn map_zeroed(
        &mut self,
        region: UserRegion,
        permissions: Permissions,
    ) -> Result<(), VmError> {
        self.ensure_available(region)?;
        for (mapped, page) in region.pages().enumerate() {
            if let Err(error) = self.map_zeroed_page(page, permissions) {
                self.rollback(region, mapped);
                return Err(error);
            }
        }

        Ok(())
    }

    fn map_zeroed_page(&mut self, page: UserPage, permissions: Permissions) -> Result<(), VmError> {
        let frame = frame::allocate_zeroed().ok_or(VmError::OutOfMemory)?;
        self.page_table
            .map_user_page(page, &frame, permissions.into())
            .map_err(mapping_error)?;
        self.pages
            .insert(page, PageState::Mapped { frame, permissions });
        Ok(())
    }

    pub(super) fn ensure_page_available(&self, page: UserPage) -> Result<(), VmError> {
        (!self.pages.contains_key(&page))
            .then_some(())
            .ok_or(VmError::AddressInUse)
    }

    fn ensure_available(&self, region: UserRegion) -> Result<(), VmError> {
        region
            .pages()
            .try_for_each(|page| self.ensure_page_available(page))
    }

    fn rollback(&mut self, region: UserRegion, mapped: usize) {
        for page in region.pages().take(mapped) {
            self.page_table.unmap_user_page(page).unwrap();
            self.pages.remove(&page).unwrap();
        }
    }
}

impl From<Permissions> for PagePermissions {
    fn from(value: Permissions) -> Self {
        match value {
            Permissions::ReadOnly => Self::ReadOnly,
            Permissions::ReadWrite => Self::ReadWrite,
            Permissions::ReadExecute => Self::ReadExecute,
        }
    }
}

fn mapping_error(error: MappingError) -> VmError {
    match error {
        MappingError::OutOfMemory => VmError::OutOfMemory,
        MappingError::AlreadyMapped => VmError::AddressInUse,
        MappingError::NotMapped => VmError::NotMapped,
        MappingError::InvalidHierarchy => VmError::MappingFailed,
    }
}
