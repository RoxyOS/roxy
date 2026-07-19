use roxy_arch::UserContext;
use roxy_thread::{Thread, ThreadCreateError, scheduler};
use roxy_vm::VmError;

use crate::{Process, ProcessId, table::PROCESS_TABLE};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForkError {
    OutOfMemory,
    InvalidAddressSpace,
}

/// Creates a child process from the current process and schedules its saved user context.
///
/// # Errors
///
/// Returns an error when the address space or child kernel stack cannot be copied or allocated.
///
/// # Panics
///
/// Panics when the current scheduled thread does not belong to a running process.
pub fn fork_current(context: UserContext) -> Result<ProcessId, ForkError> {
    let (addrspace, fds) = {
        let table = PROCESS_TABLE.lock();
        let process_id = table.current_process_id();
        let process = table.processes.get(&process_id).unwrap();
        (
            process
                .addrspace
                .clone()
                .expect("running process has no address space"),
            process.fds.clone(),
        )
    };

    let child_addrspace = addrspace.fork_copy().map_err(map_vm_error)?;
    let child_context = context.with_syscall_result(0);
    let child_thread = Thread::new_user_resume(child_context).map_err(map_thread_error)?;
    let child_process = Process::from_fork(child_thread.id(), child_addrspace, fds);
    let child_id = child_process.id;

    PROCESS_TABLE.lock().insert(child_process);
    scheduler::enqueue_user(child_thread);

    Ok(child_id)
}

fn map_vm_error(error: VmError) -> ForkError {
    match error {
        VmError::OutOfMemory => ForkError::OutOfMemory,
        VmError::InvalidRange
        | VmError::PartialUnmap
        | VmError::AddressInUse
        | VmError::NotMapped
        | VmError::MappingFailed
        | VmError::PermissionDenied => ForkError::InvalidAddressSpace,
    }
}

fn map_thread_error(error: ThreadCreateError) -> ForkError {
    match error {
        ThreadCreateError::OutOfMemory => ForkError::OutOfMemory,
    }
}
