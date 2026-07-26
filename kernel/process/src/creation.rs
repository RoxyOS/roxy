use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use roxy_thread::{Thread, ThreadCreateError, ThreadId, scheduler};
use roxy_vfs::ResolvedPath;
use roxy_vm::AddrSpaceHandle;

use crate::{
    Process, ProcessError, ProcessId, ProcessState, image, initial_fds, table::PROCESS_TABLE,
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
pub fn spawn(path: impl AsRef<[u8]>) -> Result<ProcessId, ProcessError> {
    let path = path.as_ref();
    let image = image::build(path, &[path.to_vec()], &[])?;
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
        Self {
            id: ProcessId(NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed)),
            parent_process_id: None,
            addrspace: Some(addrspace),
            main_thread_id,
            working_directory: ResolvedPath::root(),
            fds,
            pending_signals: Vec::new(),
            state: ProcessState::Running,
        }
    }

    pub(super) fn from_fork(
        parent_process_id: ProcessId,
        main_thread_id: ThreadId,
        addrspace: AddrSpaceHandle,
        working_directory: ResolvedPath,
        fds: roxy_fd::FdTable,
    ) -> Self {
        Self {
            id: ProcessId(NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed)),
            parent_process_id: Some(parent_process_id),
            addrspace: Some(addrspace),
            main_thread_id,
            working_directory,
            fds,
            pending_signals: Vec::new(),
            state: ProcessState::Running,
        }
    }
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

        assert_eq!(spawn([]), Err(ProcessError::InvalidElf));
        assert_eq!(statistics().allocated_frames, baseline);
    });
}
