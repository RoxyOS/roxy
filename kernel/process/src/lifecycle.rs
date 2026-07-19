use roxy_thread::{ThreadId, scheduler};

use crate::{
    ExitStatus, ProcessId, ProcessState,
    table::{PROCESS_TABLE, ProcessTable},
};

pub fn initialize() {
    scheduler::register_user_dispatch_hook(activate_addrspace);
    scheduler::register_reaped_handler(on_thread_reaped);
}

fn activate_addrspace(thread_id: ThreadId) {
    PROCESS_TABLE.lock().activate_addrspace(thread_id);
}

pub fn exit_current(status: ExitStatus) -> ! {
    let thread_id = scheduler::current_thread_id();
    PROCESS_TABLE.lock().begin_exit(thread_id, status);
    scheduler::exit_current()
}

pub fn take_exit_status(process_id: ProcessId) -> Option<ExitStatus> {
    PROCESS_TABLE.lock().take_exit_status(process_id)
}

fn on_thread_reaped(thread_id: ThreadId) {
    PROCESS_TABLE.lock().finish_thread_reap(thread_id);
}

impl ProcessTable {
    fn begin_exit(&mut self, thread_id: ThreadId, status: ExitStatus) {
        let process_id = self.thread_owners[&thread_id];
        let process = self.processes.get_mut(&process_id).unwrap();
        assert!(matches!(process.state, ProcessState::Running));
        process.state = ProcessState::Exiting(status);
    }

    fn finish_thread_reap(&mut self, thread_id: ThreadId) {
        let process_id = self.thread_owners.remove(&thread_id).unwrap();
        let process = self.processes.get_mut(&process_id).unwrap();
        assert_eq!(process.main_thread_id, thread_id);
        let ProcessState::Exiting(status) = process.state else {
            panic!("reaped process was not exiting");
        };

        process.addrspace = None;
        process.state = ProcessState::Exited(status);
    }

    fn take_exit_status(&mut self, process_id: ProcessId) -> Option<ExitStatus> {
        let process = self.processes.get(&process_id)?;
        let ProcessState::Exited(status) = process.state else {
            return None;
        };

        self.processes.remove(&process_id);
        Some(status)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::statistics;
    use roxy_test::kernel_test;
    use roxy_thread::Thread;
    use roxy_vm::AddrSpace;

    use super::ProcessTable;
    use crate::{ExitStatus, Process};

    kernel_test!("roxy-process::table-lifecycle", table_lifecycle, {
        let baseline = statistics().allocated_frames;
        let addrspace = AddrSpace::new().unwrap().into_handle();
        let thread = Thread::new(unused_thread).unwrap();
        let thread_id = thread.id();
        let mut table = ProcessTable::new();
        let process = Process::new(thread_id, addrspace.clone());
        let process_id = process.id;

        table.insert(process);

        table.begin_exit(thread_id, ExitStatus(42));
        table.finish_thread_reap(thread_id);
        assert_eq!(table.take_exit_status(process_id), Some(ExitStatus(42)));
        assert!(statistics().allocated_frames > baseline);

        drop(thread);
        drop(addrspace);
        assert_eq!(statistics().allocated_frames, baseline);
    });

    kernel_test!(
        "roxy-process::replace-address-space",
        replace_address_space,
        {
            let old = AddrSpace::new().unwrap().into_handle();
            let new = AddrSpace::new().unwrap().into_handle();
            let thread = Thread::new(unused_thread).unwrap();
            let thread_id = thread.id();
            let mut table = ProcessTable::new();
            let process = Process::new(thread_id, old.clone());
            let process_id = process.id;

            table.insert(process);

            let replaced = table.replace_addrspace(thread_id, new.clone());

            assert_eq!(replaced.id(), old.id());
            assert_eq!(
                table.processes[&process_id]
                    .addrspace
                    .as_ref()
                    .unwrap()
                    .id(),
                new.id()
            );
            assert_eq!(table.processes[&process_id].main_thread_id, thread_id);
            assert_eq!(table.processes[&process_id].id, process_id);
        }
    );

    fn unused_thread() -> ! {
        panic!("unused process test thread started")
    }
}
