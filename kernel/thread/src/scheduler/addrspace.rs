use core::sync::atomic::{AtomicUsize, Ordering};

use roxy_memory::{PhysicalAddress, activate_kernel_page_table, kernel_page_table_root};
use roxy_vm::AddrSpaceHandle;

static ACTIVE_ROOT: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
pub(super) enum ScheduledAddrSpace {
    Kernel,
    User(AddrSpaceHandle),
}

impl ScheduledAddrSpace {
    pub(super) fn activate_if_needed(&self) {
        let next_root = usize::try_from(self.root_address().as_u64()).unwrap();
        if ACTIVE_ROOT.swap(next_root, Ordering::AcqRel) != next_root {
            self.activate();
        }
    }

    fn root_address(&self) -> PhysicalAddress {
        match self {
            Self::Kernel => kernel_page_table_root(),
            Self::User(addrspace) => addrspace.root_address(),
        }
    }

    fn activate(&self) {
        match self {
            Self::Kernel => activate_kernel_page_table(),
            Self::User(addrspace) => addrspace.activate(),
        }
    }
}
