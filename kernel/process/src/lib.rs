#![no_std]

extern crate alloc;

mod creation;
mod lifecycle;
mod memory;
mod startup_stack;
mod table;

pub use creation::spawn;
pub use lifecycle::{exit_current, initialize, take_exit_status};
pub use memory::{
    MemoryError, allocate_anonymous, allocate_anonymous_at, free_anonymous, protect_memory,
    unmap_anonymous,
};
pub use table::current_process_id;

use alloc::sync::Arc;

use roxy_fd::{Fd, FdTable, OpenFile};
use roxy_thread::ThreadId;
use roxy_vm::AddrSpaceHandle;

/// Long-lived process metadata owned by the process table.
///
/// The scheduler separately owns the runnable thread and an address-space handle needed by the
/// context-switch path. This process retains its own handle until that thread has been safely
/// reaped on another kernel stack.
struct Process {
    id: ProcessId,
    addrspace: Option<AddrSpaceHandle>,
    main_thread_id: ThreadId,
    fds: FdTable,
    state: ProcessState,
}

enum ProcessState {
    Running,
    Exiting(ExitStatus),
    Exited(ExitStatus),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProcessId(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExitStatus(pub u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessError {
    InvalidElf,
    OutOfMemory,
    InvalidAddressSpace,
}

/// Resolves a descriptor in the currently scheduled process's FD table.
///
/// # Errors
///
/// Returns an error when the descriptor is not open.
///
/// # Panics
///
/// Panics when the current scheduled thread is not owned by a running process.
pub fn current_open_file(fd: Fd) -> Result<Arc<OpenFile>, DescriptorError> {
    let table = table::PROCESS_TABLE.lock();
    let process_id = table.current_process_id();
    let process = table.processes.get(&process_id).unwrap();
    process.fds.get(fd).ok_or(DescriptorError::NotOpen)
}

/// Inserts an open file into the current process at the lowest available descriptor.
///
/// # Panics
///
/// Panics when the current scheduled thread is not owned by a running process.
pub fn insert_open_file(file: Arc<OpenFile>) -> Fd {
    let mut table = table::PROCESS_TABLE.lock();
    let process_id = table.current_process_id();
    let process = table.processes.get_mut(&process_id).unwrap();

    process.fds.insert(file)
}

/// Closes a descriptor belonging to the currently scheduled process.
///
/// # Errors
///
/// Returns an error when the descriptor is not open.
///
/// # Panics
///
/// Panics when the current scheduled thread is not owned by a running process.
pub fn close_file(fd: Fd) -> Result<(), DescriptorError> {
    let file = {
        let mut table = table::PROCESS_TABLE.lock();
        let process_id = table.current_process_id();
        let process = table.processes.get_mut(&process_id).unwrap();
        process.fds.remove(fd)
    }
    .ok_or(DescriptorError::NotOpen)?;

    drop(file);

    Ok(())
}

/// Clones the user address space belonging to the currently scheduled process.
///
/// # Errors
///
/// Returns an error only when the current process lookup cannot resolve a descriptor context.
///
/// # Panics
///
/// Panics when the current scheduled thread is not owned by a process or its process has no
/// address space.
pub fn current_addrspace() -> Result<AddrSpaceHandle, DescriptorError> {
    let table = table::PROCESS_TABLE.lock();
    let process_id = table.current_process_id();
    let process = table.processes.get(&process_id).unwrap();

    let addrspace = process
        .addrspace
        .clone()
        .expect("running process has no address space");

    Ok(addrspace)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptorError {
    NotOpen,
}
