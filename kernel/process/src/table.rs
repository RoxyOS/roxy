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

    /// Collects the IDs of every process currently in the given process group.
    pub(super) fn process_ids_by_pgid(
        &self,
        pgid: crate::ProcessGroupId,
    ) -> alloc::vec::Vec<ProcessId> {
        self.processes
            .iter()
            .filter(|(_, process)| process.pgid == pgid)
            .map(|(_, process)| process.id)
            .collect()
    }

    /// Returns the process group of the given process.
    pub(super) fn process_pgid(&self, process_id: ProcessId) -> Option<crate::ProcessGroupId> {
        self.processes.get(&process_id).map(|process| process.pgid)
    }

    /// Returns the session of the given process.
    pub(super) fn process_session_id(&self, process_id: ProcessId) -> Option<crate::SessionId> {
        self.processes
            .get(&process_id)
            .and_then(|process| process.session_id)
    }

    /// Returns the session shared by the members of the given process group, if it is non-empty.
    ///
    /// Every member of a process group belongs to the same session, so any member's session is
    /// the group's session.
    pub(super) fn process_session_of_pgid(
        &self,
        pgid: crate::ProcessGroupId,
    ) -> Option<crate::SessionId> {
        self.processes
            .values()
            .find(|process| process.pgid == pgid)
            .and_then(|process| process.session_id)
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

    /// Clears user signal dispositions and outstanding signal frames for `execve`.
    ///
    /// Handler addresses point into the replaced image, so POSIX requires reversion to default
    /// dispositions; the mask and pending set survive.
    pub(super) fn clear_signal_actions(&mut self, thread_id: ThreadId) {
        let process_id = self.thread_owners[&thread_id];
        let process = self.processes.get_mut(&process_id).unwrap();

        process.signal_actions.clear();
        process.signal_frames.clear();
    }

    /// Closes every descriptor marked close-on-exec for `execve`.
    pub(super) fn drop_close_on_exec_fds(&mut self, thread_id: ThreadId) {
        let process_id = self.thread_owners[&thread_id];
        let process = self.processes.get_mut(&process_id).unwrap();

        process.fds.drop_close_on_exec();
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

/// Returns the current process ID when a current thread exists.
///
/// Unlike `current_process_id`, this does not panic when the scheduler has no current thread
/// (e.g. during IRQ context while the only thread is blocked).
#[must_use]
pub fn try_current_process_id() -> Option<ProcessId> {
    let thread_id = roxy_thread::scheduler::try_current_thread_id()?;
    let process_id = PROCESS_TABLE
        .lock()
        .thread_owners
        .get(&thread_id)
        .copied()?;
    Some(process_id)
}

pub fn current_parent_process_id() -> Option<ProcessId> {
    PROCESS_TABLE.lock().current_parent_process_id()
}

/// Returns the IDs of every process currently in the given process group.
pub fn process_ids_in_group(pgid: crate::ProcessGroupId) -> alloc::vec::Vec<ProcessId> {
    PROCESS_TABLE.lock().process_ids_by_pgid(pgid)
}

/// Returns the process group of the given process, if it exists.
pub fn process_pgid(process_id: ProcessId) -> Option<crate::ProcessGroupId> {
    PROCESS_TABLE.lock().process_pgid(process_id)
}

/// Returns the session of the given process, if it exists.
pub fn process_session_id(process_id: ProcessId) -> Option<crate::SessionId> {
    PROCESS_TABLE.lock().process_session_id(process_id)
}

/// Returns the session shared by the members of the given process group, if it is non-empty.
pub fn process_session_of_pgid(pgid: crate::ProcessGroupId) -> Option<crate::SessionId> {
    PROCESS_TABLE.lock().process_session_of_pgid(pgid)
}

/// Returns the session of the current process, if it has one.
pub fn current_process_session_id() -> Option<crate::SessionId> {
    let table = PROCESS_TABLE.lock();
    let process_id = table.current_process_id();
    table.processes.get(&process_id).and_then(|p| p.session_id)
}

/// Returns the current process's process group.
pub fn current_process_group_id() -> crate::ProcessGroupId {
    let table = PROCESS_TABLE.lock();
    let process_id = table.current_process_id();
    table.processes[&process_id].pgid
}

/// Reports whether the current process is a session leader (its session ID equals its PID).
#[must_use]
pub fn is_current_session_leader() -> bool {
    let table = PROCESS_TABLE.lock();
    let process_id = table.current_process_id();
    table
        .processes
        .get(&process_id)
        .is_some_and(|p| p.session_id == Some(crate::SessionId::from(process_id)))
}
