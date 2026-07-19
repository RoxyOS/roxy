use alloc::collections::BTreeMap;

use roxy_thread::ThreadId;
use roxy_utils::Lock;

use crate::{Process, ProcessId};

pub(super) static PROCESS_TABLE: Lock<ProcessTable> = Lock::new(ProcessTable::new());

pub(super) struct ProcessTable {
    pub(super) processes: BTreeMap<ProcessId, Process>,
    pub(super) thread_owners: BTreeMap<ThreadId, ProcessId>,
}

impl ProcessTable {
    pub(super) fn current_process_id(&self) -> ProcessId {
        let thread_id = roxy_thread::scheduler::current_thread_id();
        *self
            .thread_owners
            .get(&thread_id)
            .expect("thread has no process")
    }
    pub(super) const fn new() -> Self {
        Self {
            processes: BTreeMap::new(),
            thread_owners: BTreeMap::new(),
        }
    }

    pub(super) fn insert(&mut self, process: Process) {
        let previous = self
            .thread_owners
            .insert(process.main_thread_id, process.id);
        assert!(previous.is_none(), "thread already belongs to a process");
        let previous = self.processes.insert(process.id, process);
        assert!(previous.is_none(), "process id reused");
    }
}

pub fn current_process_id() -> ProcessId {
    PROCESS_TABLE.lock().current_process_id()
}
