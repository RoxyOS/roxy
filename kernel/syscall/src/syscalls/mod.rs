mod anon_allocate;
mod anon_free;
mod exit;
mod futex_wait;
mod futex_wake;
mod read;
mod write;

use crate::Syscall;

pub(super) const SYSCALLS: [Syscall; 7] = [
    exit::SYSCALL,
    read::SYSCALL,
    write::SYSCALL,
    futex_wait::SYSCALL,
    futex_wake::SYSCALL,
    anon_allocate::SYSCALL,
    anon_free::SYSCALL,
];
