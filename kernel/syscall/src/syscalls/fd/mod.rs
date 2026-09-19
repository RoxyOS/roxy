mod descriptor_flags;
mod dup;
mod dup_onto;
mod status_flags;

pub(super) const DUP_SYSCALL: crate::Syscall = dup::SYSCALL;
pub(super) const DUP_ONTO_SYSCALL: crate::Syscall = dup_onto::SYSCALL;
pub(super) const GET_DESCRIPTOR_FLAGS_SYSCALL: crate::Syscall = descriptor_flags::GET_SYSCALL;
pub(super) const SET_DESCRIPTOR_FLAGS_SYSCALL: crate::Syscall = descriptor_flags::SET_SYSCALL;
pub(super) const GET_STATUS_FLAGS_SYSCALL: crate::Syscall = status_flags::GET_SYSCALL;
pub(super) const SET_STATUS_FLAGS_SYSCALL: crate::Syscall = status_flags::SET_SYSCALL;
