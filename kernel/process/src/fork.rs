use hashbrown::HashMap;
use roxy_arch::UserContext;
use roxy_fd::FdTable;
use roxy_signal::Signal;
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
    signal_actions: HashMap<Signal, crate::SignalAction>,
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
    let child_context = child_context(context);
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
            signal_actions: process.signal_actions.clone(),
        }
    };

    let child_addrspace = snapshot.addrspace.fork_copy().map_err(map_vm_error)?;
    let child_thread = Thread::new_user_resume(child_context).map_err(map_thread_error)?;
    let child_process = Process::from_fork(
        snapshot.parent_process_id,
        child_thread.id(),
        child_addrspace,
        snapshot.working_directory,
        snapshot.fds,
        snapshot.signal_actions,
    );
    let child_id = child_process.id;

    PROCESS_TABLE.lock().insert(child_process);
    scheduler::enqueue_user(child_thread);

    Ok(child_id)
}

#[inline(never)]
fn child_context(context: UserContext) -> UserContext {
    context.with_syscall_result(0)
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

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_arch::UserContext;
    use roxy_test::kernel_test;

    use super::child_context;

    kernel_test!(
        "roxy-process::fork-child-context",
        preserves_child_resume,
        {
            let context = UserContext {
                rax: 17,
                instruction_pointer: 0x40_1234,
                flags: 0x202,
                stack_pointer: 0x7fff_ffff_e000,
                fs_base: 0x7fff_ffff_d000,
                ..UserContext::default()
            };

            assert_eq!(child_context(context), UserContext { rax: 0, ..context });
        }
    );
}
