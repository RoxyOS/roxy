#![no_std]

extern crate alloc;

mod creation;
#[cfg(feature = "kernel-test")]
mod tests;

use roxy_thread::Thread;
use roxy_vm::AddrSpace;

pub struct Process {
    _addrspace: AddrSpace,
    _main_thread: Thread,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessError {
    InvalidElf,
    OutOfMemory,
    InvalidAddressSpace,
}
