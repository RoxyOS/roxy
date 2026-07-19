mod anon_allocate;
mod anon_free;
mod clock_get;
mod exit;
mod futex_wait;
mod futex_wake;
mod read;
mod tcb_set;
mod write;

use crate::Syscall;

pub(super) const SYSCALLS: [Syscall; 9] = [
    exit::SYSCALL,
    read::SYSCALL,
    write::SYSCALL,
    futex_wait::SYSCALL,
    futex_wake::SYSCALL,
    anon_allocate::SYSCALL,
    anon_free::SYSCALL,
    tcb_set::SYSCALL,
    clock_get::SYSCALL,
];
