use alloc::vec::Vec;

use roxy_memory::{UserAddress, UserPage};

use super::{AddrSpace, PageState, Permissions, VmError, anonymous::page_count};
use crate::UserRegion;

impl AddrSpace {
    /// Changes permissions across a page-aligned mapped range.
    ///
    /// # Errors
    ///
    /// Returns an error for unaligned, invalid, or unmapped ranges.
    pub fn protect(
        &mut self,
        address: UserAddress,
        size: usize,
        permissions: Permissions,
    ) -> Result<(), VmError> {
        let start = UserPage::new(address).ok_or(VmError::InvalidRange)?;
        let region = UserRegion::new(start, page_count(size)?).ok_or(VmError::InvalidRange)?;
        let snapshot = self.permission_snapshot(region)?;

        for (index, page) in region.pages().enumerate() {
            if let Err(error) = self.protect_page(page, permissions) {
                self.rollback_permissions(region, &snapshot[..index]);

                return Err(error);
            }
        }

        Ok(())
    }

    fn permission_snapshot(&self, region: UserRegion) -> Result<Vec<Permissions>, VmError> {
        region
            .pages()
            .map(|page| match self.pages.get(&page) {
                Some(
                    PageState::Mapped { permissions, .. }
                    | PageState::MappedPhysical { permissions, .. },
                ) => Ok(*permissions),
                _ => Err(VmError::NotMapped),
            })
            .collect()
    }

    fn protect_page(&mut self, page: UserPage, permissions: Permissions) -> Result<(), VmError> {
        self.page_table
            .protect_user_page(page, permissions.into())
            .map_err(super::mapping::mapping_error)?;

        let Some(
            PageState::Mapped {
                permissions: current,
                ..
            }
            | PageState::MappedPhysical {
                permissions: current,
                ..
            },
        ) = self.pages.get_mut(&page)
        else {
            return Err(VmError::NotMapped);
        };
        *current = permissions;

        Ok(())
    }

    fn rollback_permissions(&mut self, region: UserRegion, previous: &[Permissions]) {
        for (page, permissions) in region.pages().zip(previous) {
            self.protect_page(page, *permissions)
                .expect("rollback restores a validated mapping");
        }
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::{UserAddress, UserPage};
    use roxy_test::kernel_test;

    use super::AddrSpace;
    use crate::{Permissions, VmError};

    kernel_test!("roxy-vm::protect-anonymous", protect_anonymous, {
        let mut space = AddrSpace::new().unwrap();
        let address = UserAddress::new(0x41_0000).unwrap();

        space.allocate_anonymous_at(address, 4097).unwrap();
        space
            .protect(address, 4097, Permissions::ReadExecute)
            .unwrap();

        assert_eq!(
            space.permissions(UserPage::containing(address)),
            Some(Permissions::ReadExecute)
        );
        assert_eq!(
            space.write_bytes(address, &[1]),
            Err(VmError::PermissionDenied)
        );
    });

    kernel_test!("roxy-vm::fixed-anonymous-conflict", fixed_conflict, {
        let mut space = AddrSpace::new().unwrap();
        let address = UserAddress::new(0x42_0000).unwrap();

        space.allocate_anonymous_at(address, 4096).unwrap();

        assert_eq!(
            space.allocate_anonymous_at(address, 4096),
            Err(VmError::AddressInUse)
        );
    });
}
