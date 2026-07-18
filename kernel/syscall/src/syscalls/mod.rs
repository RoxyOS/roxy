mod exit;
mod read;
mod write;

use crate::Syscall;

pub(super) const SYSCALLS: [Syscall; 3] = [exit::SYSCALL, read::SYSCALL, write::SYSCALL];
