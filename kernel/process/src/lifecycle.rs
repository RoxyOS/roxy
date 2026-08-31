use roxy_thread::{ThreadId, scheduler};

use crate::{
    ExitStatus, InitialFdInjector, ProcessState, current_umask, cwd, initial_fds,
    table::{PROCESS_TABLE, ProcessTable},
};

/// Initializes process integration and registers its VFS and descriptor hooks.
///
/// # Panics
///
/// Panics when the process subsystem was already initialized.
pub fn initialize(initial_fd_injector: InitialFdInjector) {
    initial_fds::register(initial_fd_injector);
    roxy_vfs::register_working_directory_provider(cwd::current_working_directory)
        .expect("VFS working-directory provider already registered");
    roxy_vfs::register_umask_provider(current_umask)
        .expect("VFS umask provider already registered");
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
        self.wake_exit_waiter(process_id);
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::statistics;
    use roxy_test::kernel_test;
    use roxy_thread::Thread;
    use roxy_vfs::ResolvedPath;
    use roxy_vm::AddrSpace;

    use super::ProcessTable;
    use crate::{ExitStatus, Process, ProcessGroupId};

    kernel_test!("roxy-process::table-lifecycle", table_lifecycle, {
        let baseline = statistics().allocated_frames;
        let addrspace = AddrSpace::new().unwrap().into_handle();
        let thread = Thread::new(unused_thread).unwrap();
        let thread_id = thread.id();
        let mut table = ProcessTable::new();
        let process = Process::new(thread_id, addrspace.clone(), roxy_fd::FdTable::new());
        let process_id = process.id;
        assert_eq!(process.parent_process_id, None);
        assert_eq!(process.working_directory.as_bytes(), b"/");

        let child_addrspace = AddrSpace::new().unwrap().into_handle();
        let child_thread = Thread::new(unused_thread).unwrap();
        let child_working_directory = ResolvedPath::resolve(b"/usr").unwrap();
        let child_process = Process::from_fork(
            process_id,
            ProcessGroupId::from(process_id),
            None,
            child_thread.id(),
            child_addrspace.clone(),
            child_working_directory,
            roxy_vfs::FilePermissions::DEFAULT_UMASK,
            roxy_fd::FdTable::new(),
            hashbrown::HashMap::new(),
        );
        let child_process_id = child_process.id;
        assert_eq!(child_process.parent_process_id, Some(process_id));
        assert_eq!(child_process.working_directory.as_bytes(), b"/usr");

        table.insert(process);
        table.insert(child_process);

        table.begin_exit(thread_id, ExitStatus::exited(42));
        table.finish_thread_reap(thread_id);
        assert_eq!(
            table.reap_exited_process(process_id),
            Some(ExitStatus::exited(42))
        );
        assert_eq!(table.processes[&child_process_id].parent_process_id, None);
        table.processes.remove(&child_process_id);
        assert!(statistics().allocated_frames > baseline);

        drop(thread);
        drop(addrspace);
        drop(child_thread);
        drop(child_addrspace);
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
            let process = Process::new(thread_id, old.clone(), roxy_fd::FdTable::new());
            let process_id = process.id;

            table.insert(process);

            table.set_working_directory(process_id, ResolvedPath::resolve(b"/usr").unwrap());

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
            assert_eq!(
                table.processes[&process_id].working_directory.as_bytes(),
                b"/usr"
            );
        }
    );

    fn unused_thread() -> ! {
        panic!("unused process test thread started")
    }
}
