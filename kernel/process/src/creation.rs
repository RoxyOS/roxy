use core::sync::atomic::{AtomicU64, Ordering};

use roxy_thread::{Thread, ThreadCreateError, ThreadId, scheduler};
use roxy_vm::AddrSpaceHandle;

use crate::{Process, ProcessError, ProcessId, ProcessState, image, table::PROCESS_TABLE};

static NEXT_PROCESS_ID: AtomicU64 = AtomicU64::new(1);
/// Creates a single-thread process from a VFS executable and makes it runnable.
///
/// # Errors
///
/// Returns an error for an invalid ELF image, address-space failure, or allocation failure.
pub fn spawn(path: impl AsRef<[u8]>) -> Result<ProcessId, ProcessError> {
    let path = path.as_ref();
    let image = image::build(path, &[path.to_vec()], &[])?;
    let main_thread = Thread::new_user(image.entry, image.stack_pointer).map_err(thread_error)?;
    let process = Process::new(main_thread.id(), image.addrspace);
    let process_id = process.id;

    PROCESS_TABLE.lock().insert(process);
    scheduler::enqueue_user(main_thread);

    Ok(process_id)
}

impl Process {
    pub(super) fn new(main_thread_id: ThreadId, addrspace: AddrSpaceHandle) -> Self {
        Self {
            id: ProcessId(NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed)),
            addrspace: Some(addrspace),
            main_thread_id,
            fds: roxy_fd::FdTable::new(),
            state: ProcessState::Running,
        }
    }

    pub(super) fn from_fork(
        main_thread_id: ThreadId,
        addrspace: AddrSpaceHandle,
        fds: roxy_fd::FdTable,
    ) -> Self {
        Self {
            id: ProcessId(NEXT_PROCESS_ID.fetch_add(1, Ordering::Relaxed)),
            addrspace: Some(addrspace),
            main_thread_id,
            fds,
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
