use alloc::vec::Vec;

use super::addrspace::ScheduledAddrSpace;
use crate::{SavedContext, Thread, ThreadId};

pub(super) struct Scheduler {
    pub(super) entries: Vec<SchedulerEntry>,
    pub(super) current: Option<ThreadIndex>,
    pub(super) control_context: Option<SavedContext>,
    pub(super) pending_reap: Option<ThreadIndex>,
}

pub(super) struct SchedulerEntry {
    pub(super) thread: Thread,
    pub(super) addrspace: ScheduledAddrSpace,
    pub(super) state: ThreadState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ThreadState {
    Runnable,
    Blocked,
    Exiting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ThreadIndex(pub(super) usize);

impl Scheduler {
    pub(super) const fn new() -> Self {
        Self {
            entries: Vec::new(),
            current: None,
            control_context: None,
            pending_reap: None,
        }
    }

    pub(super) fn enqueue(&mut self, thread: Thread, addrspace: ScheduledAddrSpace) {
        self.entries.push(SchedulerEntry {
            thread,
            addrspace,
            state: ThreadState::Runnable,
        });
    }

    pub(super) fn current_thread_id(&mut self) -> ThreadId {
        let current = self.current.expect("no current thread");
        self.entry(current).thread.id()
    }

    pub(super) fn entry(&mut self, index: ThreadIndex) -> &mut SchedulerEntry {
        &mut self.entries[index.0]
    }

    pub(super) fn index_of(&self, thread_id: ThreadId) -> Option<ThreadIndex> {
        self.entries
            .iter()
            .position(|entry| entry.thread.id() == thread_id)
            .map(ThreadIndex)
    }
}
