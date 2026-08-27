use roxy_memory::{PAGE_SIZE, UserAddress, UserPage};

use super::{AddrSpace, PageState, VmError};

const PAGE_BYTES: usize = 4096;

impl AddrSpace {
    /// Reads a fully mapped byte range.
    ///
    /// # Errors
    ///
    /// Returns an error before copying when any byte is outside a mapped page.
    pub fn read_bytes(&self, address: UserAddress, output: &mut [u8]) -> Result<(), VmError> {
        self.preflight(address, output.len())?;
        visit_chunks(address, output.len(), |page, offset, source| {
            let state = self.pages.get(&page).ok_or(VmError::NotMapped)?;
            let PageState::Mapped { frame, .. } = state else {
                return Err(VmError::NotMapped);
            };
            // SAFETY: AddrSpace exclusively owns every leaf frame and this is a shared read.
            unsafe { frame.read(offset, &mut output[source.clone()]) }
                .map_err(|_| VmError::MappingFailed)
        })
    }

    /// Writes a fully mapped byte range from the kernel.
    ///
    /// # Errors
    ///
    /// Returns an error before writing when any byte is outside a mapped page.
    pub fn write_bytes(&mut self, address: UserAddress, input: &[u8]) -> Result<(), VmError> {
        self.validate_writable(address, input.len())?;
        visit_chunks(address, input.len(), |page, offset, source| {
            let state = self.pages.get_mut(&page).ok_or(VmError::NotMapped)?;
            let PageState::Mapped { frame, .. } = state else {
                return Err(VmError::NotMapped);
            };

            // SAFETY: AddrSpace is mutably borrowed and exclusively owns every leaf frame.
            unsafe { frame.write(offset, &input[source]) }.map_err(|_| VmError::MappingFailed)
        })
    }

    /// Validates that an entire byte range is mapped and writable.
    ///
    /// # Errors
    ///
    /// Returns an error before mutation when the range is invalid, unmapped, or read-only.
    pub fn validate_writable(&self, address: UserAddress, length: usize) -> Result<(), VmError> {
        self.preflight(address, length)?;
        visit_chunks(address, length, |page, _, _| {
            let Some(
                PageState::Mapped { permissions, .. }
                | PageState::MappedPhysical { permissions, .. },
            ) = self.pages.get(&page)
            else {
                return Err(VmError::NotMapped);
            };

            permissions
                .writable()
                .then_some(())
                .ok_or(VmError::PermissionDenied)
        })
    }

    fn preflight(&self, address: UserAddress, length: usize) -> Result<(), VmError> {
        if length == 0 {
            return Ok(());
        }

        let last_offset = u64::try_from(length - 1).map_err(|_| VmError::InvalidRange)?;
        address
            .checked_add(last_offset)
            .ok_or(VmError::InvalidRange)?;

        visit_chunks(address, length, |page, _, _| {
            matches!(
                self.pages.get(&page),
                Some(PageState::Mapped { .. } | PageState::MappedPhysical { .. })
            )
            .then_some(())
            .ok_or(VmError::NotMapped)
        })
    }
}

fn visit_chunks(
    address: UserAddress,
    length: usize,
    mut visitor: impl FnMut(UserPage, usize, core::ops::Range<usize>) -> Result<(), VmError>,
) -> Result<(), VmError> {
    let mut consumed = 0;

    while consumed < length {
        let current = address
            .checked_add(u64::try_from(consumed).unwrap())
            .ok_or(VmError::InvalidRange)?;
        let page = UserPage::containing(current);
        let offset = usize::try_from(current.as_u64() % PAGE_SIZE).unwrap();
        let chunk = (PAGE_BYTES - offset).min(length - consumed);
        visitor(page, offset, consumed..consumed + chunk)?;
        consumed += chunk;
    }

    Ok(())
}

#[cfg(feature = "kernel-test")]
mod tests {
    use core::num::NonZeroUsize;

    use roxy_memory::{UserAddress, UserPage};
    use roxy_test::kernel_test;

    use super::{AddrSpace, VmError};
    use crate::{Permissions, UserRegion};

    kernel_test!("roxy-vm::zeroed-cross-page-io", zeroed_cross_page_io, {
        let mut space = AddrSpace::new().unwrap();
        let region = region_at(0x40_0000, 2);
        space.map_zeroed(region, Permissions::ReadWrite).unwrap();
        assert_eq!(
            space.map_zeroed(region, Permissions::ReadOnly),
            Err(VmError::AddressInUse)
        );

        let mut initial = [0xaa; 32];
        space
            .read_bytes(UserAddress::new(0x40_0ff0).unwrap(), &mut initial)
            .unwrap();
        assert_eq!(initial, [0; 32]);

        let input = *b"cross-page-user-memory";
        space
            .write_bytes(UserAddress::new(0x40_0ff8).unwrap(), &input)
            .unwrap();
        let mut output = [0; 22];
        space
            .read_bytes(UserAddress::new(0x40_0ff8).unwrap(), &mut output)
            .unwrap();
        assert_eq!(output, input);
        assert_eq!(
            space.permissions(region.start),
            Some(Permissions::ReadWrite)
        );
        space
            .read_bytes(UserAddress::new(0x70_0000).unwrap(), &mut [])
            .unwrap();
    });

    kernel_test!("roxy-vm::invalid-tail-is-atomic", invalid_tail_is_atomic, {
        let mut space = AddrSpace::new().unwrap();
        space
            .map_zeroed(region_at(0x50_0000, 1), Permissions::ReadWrite)
            .unwrap();
        let address = UserAddress::new(0x50_0ff8).unwrap();
        let input = [0x5a; 16];

        assert_eq!(space.write_bytes(address, &input), Err(VmError::NotMapped));
        let mut unchanged = [0xff; 8];
        space.read_bytes(address, &mut unchanged).unwrap();
        assert_eq!(unchanged, [0; 8]);
    });

    kernel_test!(
        "roxy-vm::read-only-tail-is-atomic",
        read_only_tail_is_atomic,
        {
            let mut space = AddrSpace::new().unwrap();
            space
                .map_zeroed(region_at(0x60_0000, 1), Permissions::ReadWrite)
                .unwrap();
            space
                .map_zeroed(region_at(0x60_1000, 1), Permissions::ReadOnly)
                .unwrap();
            let address = UserAddress::new(0x60_0ff8).unwrap();
            let input = [0x5a; 16];

            assert_eq!(
                space.validate_writable(address, input.len()),
                Err(VmError::PermissionDenied)
            );
            assert_eq!(
                space.write_bytes(address, &input),
                Err(VmError::PermissionDenied)
            );
            let mut unchanged = [0xff; 8];
            space.read_bytes(address, &mut unchanged).unwrap();
            assert_eq!(unchanged, [0; 8]);
        }
    );

    fn region_at(address: u64, pages: usize) -> UserRegion {
        let address = UserAddress::new(address).unwrap();
        let page = UserPage::new(address).unwrap();
        UserRegion::new(page, NonZeroUsize::new(pages).unwrap()).unwrap()
    }
}
