#![no_std]

extern crate alloc;

mod addrspace;
mod region;
mod stack;

pub use addrspace::{AddrSpace, Permissions, VmError};
pub use region::UserRegion;
pub use stack::UserStack;
