use roxy_thread::{ThreadId, scheduler};

use crate::{ExitStatus, ProcessId, ProcessState, table::PROCESS_TABLE, table::ProcessTable};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitTarget {
    Process(ProcessId),
    Any,
}

impl WaitTarget {
    fn is_target(self, process_id: ProcessId) -> bool {
        match self {
            Self::Process(target_process_id) => target_process_id == process_id,
            Self::Any => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitResult {
    Exited {
        process_id: ProcessId,
        status: ExitStatus,
    },
    /// A matching child exists, but none has exited; returned only by a non-blocking wait.
    Pending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitError {
    NoChild,
}

/// Waits for one matching child to exit and reaps it.
///
/// # Errors
///
/// Returns [`WaitError::NoChild`] when the current process has no matching child.
///
/// # Panics
///
/// Panics when the current process already has a registered child waiter, which violates the
/// current single-thread process model.
#[allow(
    clippy::redundant_else,
    reason = "the explicit branches distinguish non-blocking and blocking wait semantics"
)]
pub fn wait_current(target: WaitTarget, no_hang: bool) -> Result<WaitResult, WaitError> {
    loop {
        let mut table = PROCESS_TABLE.lock();
        let parent_process_id = table.current_process_id();
        table.child_waiters.remove(&parent_process_id);

        if let Some(process_id) = table.find_exited_matching_child(parent_process_id, target) {
            let status = table.reap_exited_process(process_id).unwrap();

            return Ok(WaitResult::Exited { process_id, status });
        }

        if !table.has_matching_child(parent_process_id, target) {
            return Err(WaitError::NoChild);
        }

        if no_hang {
            return Ok(WaitResult::Pending);
        } else {
            let previous = table.child_waiters.insert(parent_process_id, target);

            assert!(previous.is_none(), "process already has a child waiter");

            let pending = scheduler::prepare_block_current();
            drop(table);

            pending.perform();
        }
    }
}

impl ProcessTable {
    fn find_exited_matching_child(
        &self,
        parent_process_id: ProcessId,
        target: WaitTarget,
    ) -> Option<ProcessId> {
        self.processes.iter().find_map(|(process_id, process)| {
            (process.parent_process_id == Some(parent_process_id)
                && target.is_target(*process_id)
                && matches!(process.state, ProcessState::Exited(_)))
            .then_some(*process_id)
        })
    }

    fn has_matching_child(&self, parent_process_id: ProcessId, target: WaitTarget) -> bool {
        self.processes.iter().any(|(process_id, process)| {
            process.parent_process_id == Some(parent_process_id) && target.is_target(*process_id)
        })
    }

    /// Wakes the process waiting for `process_id` to exit.
    pub(super) fn wake_waiter(&mut self, process_id: ProcessId) {
        let Some(thread_id) = self.take_waiter_thread(process_id) else {
            return;
        };

        assert!(scheduler::wake(thread_id), "child waiter was not blocked");
    }

    /// Returns the thread ID of the process waiting for `process_id` to exit and removes its
    /// waiter.
    fn take_waiter_thread(&mut self, process_id: ProcessId) -> Option<ThreadId> {
        let process = &self.processes[&process_id];
        assert!(matches!(process.state, ProcessState::Exited(_)));
        let parent_process_id = process.parent_process_id?;
        let target = self.child_waiters.get(&parent_process_id)?;

        if !target.is_target(process_id) {
            return None;
        }

        self.child_waiters.remove(&parent_process_id);

        Some(self.processes[&parent_process_id].main_thread_id)
    }

    pub(super) fn reap_exited_process(&mut self, process_id: ProcessId) -> Option<ExitStatus> {
        let process = self.processes.get(&process_id)?;
        let ProcessState::Exited(status) = process.state else {
            return None;
        };

        self.processes.remove(&process_id);
        self.child_waiters.remove(&process_id);

        for process in self.processes.values_mut() {
            if process.parent_process_id == Some(process_id) {
                process.parent_process_id = None;
            }
        }

        Some(status)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;
    use roxy_thread::Thread;
    use roxy_vfs::ResolvedPath;
    use roxy_vm::AddrSpace;

    use super::WaitTarget;
    use crate::{ExitStatus, Process, ProcessState, table::ProcessTable};

    kernel_test!("roxy-process::wait-selection", wait_selection, {
        let parent_thread = Thread::new(unused_thread).unwrap();
        let parent = Process::new(
            parent_thread.id(),
            AddrSpace::new().unwrap().into_handle(),
            roxy_fd::FdTable::new(),
        );
        let parent_id = parent.id;
        let first_thread = Thread::new(unused_thread).unwrap();
        let first = child(parent_id, first_thread.id(), ProcessState::Running);
        let first_id = first.id;
        let second_thread = Thread::new(unused_thread).unwrap();
        let second_status = ExitStatus::new(22);
        let second = child(
            parent_id,
            second_thread.id(),
            ProcessState::Exited(second_status),
        );
        let second_id = second.id;
        let third_thread = Thread::new(unused_thread).unwrap();
        let third_status = ExitStatus::new(33);
        let third = child(
            parent_id,
            third_thread.id(),
            ProcessState::Exited(third_status),
        );
        let third_id = third.id;
        let mut table = ProcessTable::new();

        table.insert(parent);
        table.insert(first);
        table.insert(second);
        table.insert(third);

        table
            .child_waiters
            .insert(parent_id, WaitTarget::Process(third_id));
        assert_eq!(table.take_waiter_thread(second_id), None);
        assert_eq!(
            table.child_waiters.get(&parent_id),
            Some(&WaitTarget::Process(third_id))
        );
        assert_eq!(table.take_waiter_thread(third_id), Some(parent_thread.id()));
        table.child_waiters.insert(parent_id, WaitTarget::Any);
        assert_eq!(
            table.take_waiter_thread(second_id),
            Some(parent_thread.id())
        );

        assert_eq!(
            table.find_exited_matching_child(parent_id, WaitTarget::Any),
            Some(second_id)
        );
        assert_eq!(table.reap_exited_process(second_id), Some(second_status));
        assert_eq!(
            table.find_exited_matching_child(parent_id, WaitTarget::Process(third_id)),
            Some(third_id)
        );
        assert_eq!(table.reap_exited_process(third_id), Some(third_status));
        assert_eq!(
            table.find_exited_matching_child(parent_id, WaitTarget::Any),
            None
        );
        assert!(table.has_matching_child(parent_id, WaitTarget::Any));
        assert!(!table.has_matching_child(parent_id, WaitTarget::Process(second_id)));

        table.processes.get_mut(&first_id).unwrap().state =
            ProcessState::Exited(ExitStatus::new(11));
        assert_eq!(
            table.find_exited_matching_child(parent_id, WaitTarget::Process(first_id)),
            Some(first_id)
        );
        assert_eq!(
            table.reap_exited_process(first_id),
            Some(ExitStatus::new(11))
        );
        assert!(!table.has_matching_child(parent_id, WaitTarget::Any));
    });

    fn child(
        parent_id: crate::ProcessId,
        thread_id: roxy_thread::ThreadId,
        state: ProcessState,
    ) -> Process {
        let mut process = Process::from_fork(
            parent_id,
            thread_id,
            AddrSpace::new().unwrap().into_handle(),
            ResolvedPath::root(),
            roxy_fd::FdTable::new(),
        );
        process.state = state;

        process
    }

    fn unused_thread() -> ! {
        panic!("unused process test thread started")
    }
}
