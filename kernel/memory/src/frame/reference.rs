use alloc::sync::Arc;

use crate::{PhysicalAddress, address::PAGE_SIZE};

use super::allocator;

pub struct OwnedFrame {
    frame: usize,
    owned: bool,
}

impl OwnedFrame {
    pub(super) const fn new(frame: usize) -> Self {
        Self { frame, owned: true }
    }

    #[must_use]
    pub fn start_address(&self) -> PhysicalAddress {
        frame_address(self.frame)
    }

    #[must_use]
    pub fn into_page_ref(mut self) -> PageRef {
        self.owned = false;
        PageRef(Arc::new(FrameOwner { frame: self.frame }))
    }

    pub(crate) fn transfer_to_mapping(mut self) {
        self.owned = false;
    }
}

impl Drop for OwnedFrame {
    fn drop(&mut self) {
        if self.owned {
            allocator::deallocate(self.frame);
        }
    }
}

#[derive(Clone)]
pub struct PageRef(Arc<FrameOwner>);

impl PageRef {
    #[must_use]
    pub fn start_address(&self) -> PhysicalAddress {
        frame_address(self.0.frame)
    }
}

struct FrameOwner {
    frame: usize,
}

impl Drop for FrameOwner {
    fn drop(&mut self) {
        allocator::deallocate(self.frame);
    }
}

fn frame_address(frame: usize) -> PhysicalAddress {
    let address = u64::try_from(frame)
        .unwrap()
        .checked_mul(PAGE_SIZE)
        .unwrap();
    PhysicalAddress::new(address).unwrap()
}
