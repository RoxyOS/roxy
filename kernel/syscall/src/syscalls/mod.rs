mod exit;

use crate::Syscall;

pub(super) const SYSCALLS: [Syscall; 1] = [exit::SYSCALL];
