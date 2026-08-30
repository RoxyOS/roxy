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
mod signal_frame;
pub use signal_frame::SIGRETURN_SYSCALL_NUMBER;
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
    MemoryError, allocate_anonymous, allocate_anonymous_at, free_anonymous, map_physical,
    protect_memory, unmap_anonymous, unmap_memory,
};
use signal::PendingSignal;
pub use signal::{
    SignalAction, SignalError, block_signals, currently_blocked_signals, deliver_pending_signal,
    has_pending_signal, pop_signal_frame, replace_masked_signals, replace_signal_action,
    send_signal, signal_action_of, unblock_signals,
};
pub use table::{current_parent_process_id, current_process_id};
pub use wait::{WaitError, WaitResult, WaitTarget, wait_current};

use alloc::{sync::Arc, vec::Vec};

use hashbrown::HashMap;
use roxy_fd::{DupError, Fd, FdTable, OpenFile};
use roxy_signal::{Signal, SignalSet};
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
    pending_signals: Vec<PendingSignal>,
    masked_signals: SignalSet,
    signal_frames: Vec<u64>,
    signal_actions: HashMap<Signal, SignalAction>,
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
/// `close_on_exec` records whether `execve` should close the descriptor.
///
/// # Panics
///
/// Panics when the current scheduled thread is not owned by a running process.
pub fn insert_open_file(file: Arc<OpenFile>, close_on_exec: bool) -> Fd {
    let mut table = table::PROCESS_TABLE.lock();
    let process_id = table.current_process_id();
    let process = table.processes.get_mut(&process_id).unwrap();

    process.fds.insert(file, close_on_exec)
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

/// Makes `newfd` refer to the same open file description as `oldfd` in the currently
/// scheduled process.
///
/// `close_on_exec` records whether `execve` should close the new descriptor.
///
/// # Errors
///
/// Returns an error when `oldfd` is not open.
///
/// # Panics
///
/// Panics when the current scheduled thread is not owned by a running process.
pub fn dup2_current(oldfd: Fd, newfd: Fd, close_on_exec: bool) -> Result<(), DescriptorError> {
    let mut table = table::PROCESS_TABLE.lock();
    let process_id = table.current_process_id();
    let process = table.processes.get_mut(&process_id).unwrap();

    process
        .fds
        .dup2(oldfd, newfd, close_on_exec)
        .map_err(map_dup_error)
}

fn map_dup_error(_: DupError) -> DescriptorError {
    DescriptorError::NotOpen
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
