use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use roxy_memory::UserAddress;
use roxy_signal::SignalSet;

use hashbrown::HashMap;
use roxy_thread::{Thread, ThreadCreateError, ThreadId, scheduler};
use roxy_vfs::{FilePermissions, ResolvedPath};
use roxy_vm::AddrSpaceHandle;

use crate::{
    Process, ProcessError, ProcessGroupId, ProcessId, ProcessState, SessionId, image, initial_fds,
    table::PROCESS_TABLE,
};

static NEXT_PROCESS_ID: AtomicU64 = AtomicU64::new(1);
/// Creates a single-thread process from a VFS executable and makes it runnable.
///
/// # Errors
///
/// Returns an error for an invalid ELF image, address-space failure, or allocation failure.
///
/// # Panics
///
/// Panics when the process subsystem has no registered initial-FD injector.
pub fn spawn(path: impl AsRef<[u8]>, envp: &[Vec<u8>]) -> Result<ProcessId, ProcessError> {
    let path = path.as_ref();
    let image = image::build(path, &[path.to_vec()], envp)?;
    let main_thread = Thread::new_user(image.entry, image.stack_pointer).map_err(thread_error)?;
    let mut fds = roxy_fd::FdTable::new();

    initial_fds::inject(&mut fds);

    let process = Process::new(main_thread.id(), image.addrspace, fds);
    let process_id = process.id;

    PROCESS_TABLE.lock().insert(process);
    scheduler::enqueue_user(main_thread);

    Ok(process_id)
}

impl Process {
    pub(super) fn new(
        main_thread_id: ThreadId,
        addrspace: AddrSpaceHandle,
        fds: roxy_fd::FdTable,
    ) -> Self {
        let id = ProcessId(NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed));
        let pgid = ProcessGroupId::from(id);
        // Every directly spawned process starts a new session (init/getty model):
        // it is both its own group leader and its own session leader.
        let session_id = Some(SessionId::from(id));

        Self {
            id,
            pgid,
            session_id,
            parent_process_id: None,
            addrspace: Some(addrspace),
            main_thread_id,
            working_directory: ResolvedPath::root(),
            umask: FilePermissions::DEFAULT_UMASK,
            fds,
            pending_signals: Vec::new(),
            masked_signals: SignalSet::empty(),
            signal_frames: Vec::new(),
            signal_actions: HashMap::new(),
            state: ProcessState::Running,
            continued: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_fork(
        parent_process_id: ProcessId,
        pgid: ProcessGroupId,
        session_id: Option<SessionId>,
        main_thread_id: ThreadId,
        addrspace: AddrSpaceHandle,
        working_directory: ResolvedPath,
        umask: FilePermissions,
        fds: roxy_fd::FdTable,
        signal_actions: HashMap<roxy_signal::Signal, crate::SignalAction>,
    ) -> Self {
        Self {
            id: ProcessId(NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed)),
            pgid,
            session_id,
            parent_process_id: Some(parent_process_id),
            addrspace: Some(addrspace),
            main_thread_id,
            working_directory,
            umask,
            fds,
            pending_signals: Vec::new(),
            masked_signals: SignalSet::empty(),
            signal_frames: Vec::new(),
            signal_actions,
            state: ProcessState::Running,
            continued: false,
        }
    }
}

/// Creates a user thread in the currently scheduled process and makes it runnable.
///
/// The new thread shares the process's address space, descriptor table, and signal state. It
/// begins at `entry` on the already-mapped user stack described by `stack_pointer`; the caller
/// (typically a future thread-create syscall or the runtime) is responsible for the user stack.
///
/// # Errors
///
/// Returns an error when the new thread's kernel stack cannot be allocated.
///
/// # Panics
///
/// Panics when the current scheduled thread is not owned by a running process.
pub fn create_thread(
    entry: UserAddress,
    stack_pointer: UserAddress,
) -> Result<ThreadId, ThreadCreateError> {
    let thread = Thread::new_user(entry, stack_pointer)?;
    let thread_id = thread.id();
    {
        let mut table = PROCESS_TABLE.lock();
        let process_id = table.current_process_id();
        table.attach_thread(thread_id, process_id);
    }
    scheduler::enqueue_user(thread);

    Ok(thread_id)
}

fn thread_error(error: ThreadCreateError) -> ProcessError {
    match error {
        ThreadCreateError::OutOfMemory => ProcessError::OutOfMemory,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::statistics;
    use roxy_test::kernel_test;

    use super::spawn;
    use crate::ProcessError;

    kernel_test!("roxy-process::reject-invalid-elf", reject_invalid_elf, {
        let baseline = statistics().allocated_frames;

        assert_eq!(spawn([], &[]), Err(ProcessError::InvalidElf));
        assert_eq!(statistics().allocated_frames, baseline);
    });
}
