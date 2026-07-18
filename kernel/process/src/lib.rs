#![no_std]

extern crate alloc;

mod creation;
mod lifecycle;
mod table;

pub use creation::spawn;
pub use lifecycle::{exit_current, initialize, take_exit_status};

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
