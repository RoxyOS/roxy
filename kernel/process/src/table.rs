use alloc::collections::BTreeMap;

use roxy_thread::ThreadId;
use roxy_utils::Lock;
use roxy_vm::AddrSpaceHandle;

use crate::{Process, ProcessId, WaitTarget};

pub(super) static PROCESS_TABLE: Lock<ProcessTable> = Lock::new(ProcessTable::new());

pub(super) struct ProcessTable {
    pub(super) processes: BTreeMap<ProcessId, Process>,
    pub(super) thread_owners: BTreeMap<ThreadId, ProcessId>,
    /// `ProcessId` is waiting for `WaitTarget`
    pub(super) child_waiters: BTreeMap<ProcessId, WaitTarget>,
}

impl ProcessTable {
    pub(super) fn current_process_id(&self) -> ProcessId {
        let thread_id = roxy_thread::scheduler::current_thread_id();
        *self
            .thread_owners
            .get(&thread_id)
            .expect("thread has no process")
    }

    pub(super) fn current_process(&mut self) -> Option<&mut Process> {
        let thread_id = roxy_thread::scheduler::try_current_thread_id()?;
        let process_id = *self.thread_owners.get(&thread_id)?;

        self.processes.get_mut(&process_id)
    }

    pub(super) fn current_parent_process_id(&self) -> Option<ProcessId> {
        let process_id = self.current_process_id();

        self.processes[&process_id].parent_process_id
    }

    pub(super) const fn new() -> Self {
        Self {
            processes: BTreeMap::new(),
            thread_owners: BTreeMap::new(),
            child_waiters: BTreeMap::new(),
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

    pub(super) fn replace_addrspace(
        &mut self,
        thread_id: ThreadId,
        addrspace: AddrSpaceHandle,
    ) -> AddrSpaceHandle {
        let process_id = self.thread_owners[&thread_id];
        let process = self.processes.get_mut(&process_id).unwrap();

        process.addrspace.replace(addrspace).unwrap()
    }

    pub(super) fn activate_addrspace(&self, thread_id: ThreadId) {
        let process_id = self.thread_owners[&thread_id];
        let addrspace = self.processes[&process_id]
            .addrspace
            .as_ref()
            .expect("running process has no address space");

        addrspace.activate();
    }
}

pub fn current_process_id() -> ProcessId {
    PROCESS_TABLE.lock().current_process_id()
}

pub fn current_parent_process_id() -> Option<ProcessId> {
    PROCESS_TABLE.lock().current_parent_process_id()
}
