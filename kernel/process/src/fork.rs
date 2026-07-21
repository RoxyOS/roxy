use roxy_arch::UserContext;
use roxy_fd::FdTable;
use roxy_thread::{Thread, ThreadCreateError, scheduler};
use roxy_vfs::ResolvedPath;
use roxy_vm::VmError;

use crate::{Process, ProcessId, table::PROCESS_TABLE};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ForkError {
    OutOfMemory,
    InvalidAddressSpace,
}

/// Process-owned state copied while creating a fork child.
struct ForkInfo {
    parent_process_id: ProcessId,
    addrspace: roxy_vm::AddrSpaceHandle,
    fds: FdTable,
    working_directory: ResolvedPath,
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
    let snapshot = {
        let table = PROCESS_TABLE.lock();
        let process_id = table.current_process_id();
        let process = table.processes.get(&process_id).unwrap();

        ForkInfo {
            parent_process_id: process_id,
            addrspace: process
                .addrspace
                .clone()
                .expect("running process has no address space"),
            fds: process.fds.clone(),
            working_directory: process.working_directory.clone(),
        }
    };

    let child_addrspace = snapshot.addrspace.fork_copy().map_err(map_vm_error)?;
    let child_context = context.with_syscall_result(0);
    let child_thread = Thread::new_user_resume(child_context).map_err(map_thread_error)?;
    let child_process = Process::from_fork(
        snapshot.parent_process_id,
        child_thread.id(),
        child_addrspace,
        snapshot.working_directory,
        snapshot.fds,
    );
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
