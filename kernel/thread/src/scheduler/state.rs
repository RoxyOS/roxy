use alloc::vec::Vec;

use super::WaitKey;
use crate::{SavedContext, Thread, ThreadId};

pub(super) struct Scheduler {
    pub(super) entries: Vec<SchedulerEntry>,
    pub(super) current: Option<ThreadIndex>,
    pub(super) control_context: Option<SavedContext>,
    pub(super) pending_reap: Option<ThreadIndex>,
}

pub(super) struct SchedulerEntry {
    pub(super) thread: Thread,
    pub(super) kind: ThreadKind,
    pub(super) state: ThreadState,
}

#[derive(Clone, Copy)]
pub(super) enum ThreadKind {
    Kernel,
    User,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ThreadState {
    Runnable,
    Blocked(BlockState),
    Exiting,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BlockState {
    Unkeyed,
    Keyed(WaitKey),
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

    pub(super) fn enqueue(&mut self, thread: Thread, kind: ThreadKind) {
        self.entries.push(SchedulerEntry {
            thread,
            kind,
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
