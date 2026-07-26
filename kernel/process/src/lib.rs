#![no_std]

extern crate alloc;

mod creation;
mod cwd;
mod execve;
mod fork;
mod image;
mod initial_fds;
mod lifecycle;
mod memory;
mod signal;
mod startup_stack;
mod table;
mod wait;

pub use creation::spawn;
pub use cwd::{current_working_directory, set_current_working_directory};
pub use execve::execve_current;
pub use fork::{ForkError, fork_current};
pub use initial_fds::InitialFdInjector;
pub use lifecycle::{exit_current, initialize};
pub use memory::{
    MemoryError, allocate_anonymous, allocate_anonymous_at, free_anonymous, protect_memory,
    unmap_anonymous,
};
pub use signal::{SignalError, process_latest_signal, send_signal};
pub use table::{current_parent_process_id, current_process_id};
pub use wait::{WaitError, WaitResult, WaitTarget, wait_current};

use alloc::{sync::Arc, vec::Vec};

use roxy_fd::{Fd, FdTable, OpenFile};
use roxy_signal::Signal;
use roxy_thread::ThreadId;
use roxy_vfs::ResolvedPath;
use roxy_vm::AddrSpaceHandle;

/// Long-lived process metadata owned by the process table.
///
/// The scheduler resolves this process-owned address space only while preparing a context switch.
/// The process retains it until its thread has been safely reaped on another kernel stack.
struct Process {
    id: ProcessId,
    parent_process_id: Option<ProcessId>,
    addrspace: Option<AddrSpaceHandle>,
    main_thread_id: ThreadId,
    working_directory: ResolvedPath,
    fds: FdTable,
    pending_signals: Vec<Signal>,
    state: ProcessState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProcessState {
    Running,
    Exiting(ExitStatus),
    Exited(ExitStatus),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProcessId(u64);

impl ProcessId {
    #[must_use]
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitStatus {
    Exited(u8),
    Signaled(Signal),
}

impl ExitStatus {
    #[must_use]
    pub fn exited(raw: u64) -> Self {
        Self::Exited(raw.to_le_bytes()[0])
    }

    #[must_use]
    pub const fn signaled(signal: Signal) -> Self {
        Self::Signaled(signal)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessError {
    ArgumentsTooLarge,
    FileNotFound,
    InvalidElf,
    InvalidAddressSpace,
    OutOfMemory,
    UnsupportedElf,
    UnsupportedFile,
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
