use alloc::collections::{BTreeMap, VecDeque};

use roxy_memory::UserAddress;
use roxy_thread::ThreadId;
use roxy_vm::AddrSpaceId;

use crate::FutexError;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct FutexKey {
    addrspace: AddrSpaceId,
    address: u64,
}

impl FutexKey {
    pub(super) fn new(addrspace: AddrSpaceId, address: UserAddress) -> Result<Self, FutexError> {
        address
            .as_u64()
            .is_multiple_of(4)
            .then_some(Self {
                addrspace,
                address: address.as_u64(),
            })
            .ok_or(FutexError::Invalid)
    }
}

pub(super) struct FutexTable {
    waiters: BTreeMap<FutexKey, VecDeque<ThreadId>>,
}

impl FutexTable {
    pub(super) const fn new() -> Self {
        Self {
            waiters: BTreeMap::new(),
        }
    }

    pub(super) fn enqueue(&mut self, key: FutexKey, thread_id: ThreadId) {
        self.waiters.entry(key).or_default().push_back(thread_id);
    }

    pub(super) fn dequeue(&mut self, key: FutexKey) -> Option<ThreadId> {
        let waiters = self.waiters.get_mut(&key)?;
        let thread_id = waiters.pop_front();
        if waiters.is_empty() {
            self.waiters.remove(&key);
        }
        thread_id
    }

    pub(super) fn remove(&mut self, thread_id: ThreadId) {
        self.waiters.retain(|_, waiters| {
            waiters.retain(|candidate| *candidate != thread_id);
            !waiters.is_empty()
        });
    }
}
