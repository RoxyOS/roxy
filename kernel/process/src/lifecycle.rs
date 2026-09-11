use roxy_thread::{ThreadId, scheduler};

use crate::{
    ExitStatus, InitialFdInjector, ProcessId, ProcessState, SessionId, current_umask, cwd,
    initial_fds,
    table::{PROCESS_TABLE, ProcessTable},
};

/// Invoked when a session leader exits, so its controlling terminal can send SIGHUP to the
/// foreground process group and release the session.
///
/// Registered by the terminal subsystem (reverse dependency: process does not depend on tty).
pub type SessionLeaderExitHandler = fn(SessionId);

static SESSION_LEADER_EXIT_HANDLER: spin::Once<SessionLeaderExitHandler> = spin::Once::new();

/// Registers the handler invoked when a session leader exits.
///
/// # Panics
///
/// Panics when a handler was already registered.
pub fn register_session_leader_exit_handler(handler: SessionLeaderExitHandler) {
    assert!(
        SESSION_LEADER_EXIT_HANDLER.get().is_none(),
        "session leader exit handler already registered"
    );
    SESSION_LEADER_EXIT_HANDLER.call_once(|| handler);
}

fn notify_session_leader_exit(session: SessionId) {
    let handler = SESSION_LEADER_EXIT_HANDLER
        .get()
        .expect("session leader exit handler must be registered before a session leader exits");
    handler(session);
}

/// Invoked when a process starts exiting, so subsystems holding resources on its behalf can
/// release them.
///
/// Registered by the resource owner (reverse dependency: process does not depend on its clients).
/// A handler runs after the process table is unlocked, so it may use process queries; the exiting
/// process ID is still passed because the caller already has it.
pub type ProcessExitHandler = fn(ProcessId);

static PROCESS_EXIT_HANDLER: spin::Once<ProcessExitHandler> = spin::Once::new();

/// Registers the handler invoked when a process starts exiting.
///
/// # Panics
///
/// Panics when a handler was already registered.
pub fn register_process_exit_handler(handler: ProcessExitHandler) {
    assert!(
        PROCESS_EXIT_HANDLER.get().is_none(),
        "process exit handler already registered"
    );
    PROCESS_EXIT_HANDLER.call_once(|| handler);
}

fn notify_process_exit(process: ProcessId) {
    if let Some(handler) = PROCESS_EXIT_HANDLER.get() {
        handler(process);
    }
}

/// What a process's transition to `Exiting` obliges its caller to publish once the process-table
/// lock is released.
struct ExitNotification {
    process: ProcessId,
    session: Option<SessionId>,
}

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
    let exited = PROCESS_TABLE.lock().begin_exit(thread_id, status);
    notify_process_exit(exited.process);
    if let Some(session) = exited.session {
        notify_session_leader_exit(session);
    }
    scheduler::exit_current()
}

/// Ends only the current thread, leaving the process running while other threads remain.
///
/// Terminating the last thread of the process also terminates the process (with a default exit
/// status), matching POSIX semantics that a process lives until its last thread ends. The reap
/// on another stack handles both cases: a non-final thread reap keeps the process alive, while
/// a final-thread reap requires this function to have marked the process `Exiting` first.
pub fn thread_exit_current() -> ! {
    let thread_id = scheduler::current_thread_id();
    let is_last = {
        let table = PROCESS_TABLE.lock();
        let process_id = table.current_process_id();
        table.is_last_thread(process_id, thread_id)
    };
    if is_last {
        let exited = PROCESS_TABLE
            .lock()
            .begin_exit(thread_id, ExitStatus::exited(0));
        notify_process_exit(exited.process);
        if let Some(session) = exited.session {
            notify_session_leader_exit(session);
        }
    }
    scheduler::exit_current()
}

fn on_thread_reaped(thread_id: ThreadId) {
    PROCESS_TABLE.lock().finish_thread_reap(thread_id);
}

impl ProcessTable {
    /// Marks a process as exiting and reports what the caller must publish.
    ///
    /// The caller publishes the notification after this call returns, so no handler runs while the
    /// process table is locked.
    fn begin_exit(&mut self, thread_id: ThreadId, status: ExitStatus) -> ExitNotification {
        let process_id = self.thread_owners[&thread_id];
        let process = self.processes.get_mut(&process_id).unwrap();
        assert!(matches!(process.state, ProcessState::Running));
        process.state = ProcessState::Exiting(status);

        let is_session_leader = process.session_id == Some(SessionId::from(process_id));

        ExitNotification {
            process: process_id,
            session: is_session_leader.then_some(process.session_id).flatten(),
        }
    }

    fn finish_thread_reap(&mut self, thread_id: ThreadId) {
        let process_id = self.thread_owners.remove(&thread_id).unwrap();

        // Free the reaped thread's per-thread signal state so a blocked `sigtimedwait` or a
        // pending `SIGEV_THREAD_ID` signal cannot outlive its thread.
        if let Some(process) = self.processes.get_mut(&process_id) {
            process.thread_masks.remove(&thread_id);
            process.thread_pending.remove(&thread_id);
        }

        if self.process_has_threads(process_id) {
            // A non-final thread was reaped. The process keeps its address space, descriptor
            // table, and signal state; only this thread is gone, so the process continues.
            // TODO(missing-capability: thread-teardown): a process-level exit should stop and
            // reap its sibling threads rather than waiting for each to reach this path on its
            // own, so a process whose main thread exits with siblings still running finalizes
            // promptly (POSIX exit semantics).
            return;
        }

        let process = self.processes.get_mut(&process_id).unwrap();
        let ProcessState::Exiting(status) = process.state else {
            panic!("final thread of a running process was reaped while not exiting");
        };
        process.addrspace = None;
        process.state = ProcessState::Exited(status);
        self.wake_state_waiter(process_id);
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
    use crate::{ExitStatus, Process, ProcessGroupId, ProcessState};

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

    kernel_test!(
        "roxy-process::secondary-thread-reap-keeps-process",
        reap_secondary,
        {
            let addrspace = AddrSpace::new().unwrap().into_handle();
            let main_thread = Thread::new(unused_thread).unwrap();
            let main_id = main_thread.id();
            let mut table = ProcessTable::new();
            let process = Process::new(main_id, addrspace.clone(), roxy_fd::FdTable::new());
            let process_id = process.id;
            table.insert(process);

            let secondary = Thread::new(unused_thread).unwrap();
            let secondary_id = secondary.id();
            table.attach_thread(secondary_id, process_id);

            // Reaping a secondary thread leaves the process running with the main thread intact.
            table.finish_thread_reap(secondary_id);
            assert_eq!(table.processes[&process_id].state, ProcessState::Running);
            assert!(table.process_has_threads(process_id));

            // Process-directed signal delivery falls back to the remaining main thread.
            assert_eq!(table.signal_target_thread(process_id), main_id);

            // The final-thread reap finalizes the process with the recorded status.
            table.begin_exit(main_id, ExitStatus::exited(7));
            table.finish_thread_reap(main_id);
            assert_eq!(
                table.reap_exited_process(process_id),
                Some(ExitStatus::exited(7))
            );

            drop(main_thread);
            drop(secondary);
            drop(addrspace);
        }
    );

    kernel_test!(
        "roxy-process::signal-target-survives-main-thread",
        signal_target,
        {
            let addrspace = AddrSpace::new().unwrap().into_handle();
            let main_thread = Thread::new(unused_thread).unwrap();
            let main_id = main_thread.id();
            let mut table = ProcessTable::new();
            let process = Process::new(main_id, addrspace.clone(), roxy_fd::FdTable::new());
            let process_id = process.id;
            table.insert(process);

            let secondary = Thread::new(unused_thread).unwrap();
            let secondary_id = secondary.id();
            table.attach_thread(secondary_id, process_id);

            // Simulate the main thread having reaped while a secondary thread remains.
            assert_eq!(table.thread_owners.remove(&main_id), Some(process_id));
            assert_eq!(table.signal_target_thread(process_id), secondary_id);

            drop(main_thread);
            drop(secondary);
            drop(addrspace);
        }
    );

    fn unused_thread() -> ! {
        panic!("unused process test thread started")
    }
}
