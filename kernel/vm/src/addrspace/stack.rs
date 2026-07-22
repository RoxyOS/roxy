use core::num::NonZeroUsize;

use roxy_memory::{PAGE_SIZE, UserAddress, UserPage};

use super::{AddrSpace, PageState, Permissions, VmError};
use crate::{UserRegion, UserStack};

const STACK_PAGES: usize = 16;
const STACK_TOP: u64 = 0x0000_7fff_ffff_f000;

impl AddrSpace {
    /// Creates the fixed 64 KiB user stack and its lower guard page.
    ///
    /// # Errors
    ///
    /// Returns an error on overlap or allocation failure.
    pub fn map_stack(&mut self) -> Result<UserStack, VmError> {
        let top = UserAddress::new(STACK_TOP).ok_or(VmError::InvalidRange)?;
        let stack_bytes = u64::try_from(STACK_PAGES)
            .ok()
            .and_then(|pages| pages.checked_mul(PAGE_SIZE))
            .ok_or(VmError::InvalidRange)?;
        let bottom = top
            .as_u64()
            .checked_sub(stack_bytes)
            .and_then(UserAddress::new)
            .ok_or(VmError::InvalidRange)?;
        let start = UserPage::new(bottom).ok_or(VmError::InvalidRange)?;
        let guard = bottom
            .as_u64()
            .checked_sub(PAGE_SIZE)
            .and_then(UserAddress::new)
            .and_then(UserPage::new)
            .ok_or(VmError::InvalidRange)?;
        let page_count = NonZeroUsize::new(STACK_PAGES).ok_or(VmError::InvalidRange)?;
        let region = UserRegion::new(start, page_count).ok_or(VmError::InvalidRange)?;

        self.ensure_page_available(guard)?;
        self.pages.insert(guard, PageState::Guard);

        if let Err(error) = self.map_zeroed(region, Permissions::ReadWrite) {
            self.pages.remove(&guard);
            return Err(error);
        }

        Ok(UserStack::new(bottom, top, guard))
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::UserPage;
    use roxy_test::kernel_test;

    use super::{AddrSpace, VmError};

    kernel_test!("roxy-vm::stack-has-guard", stack_has_guard, {
        let mut space = AddrSpace::new().unwrap();
        let stack = space.map_stack().unwrap();

        assert!(space.is_mapped(UserPage::containing(stack.bottom)));
        assert!(!space.is_mapped(stack.guard_page));
        assert_eq!(space.map_stack(), Err(VmError::AddressInUse));
    });
}
