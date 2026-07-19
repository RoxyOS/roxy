mod anon_allocate;
mod anon_free;
mod clock_get;
mod close;
mod exit;
mod fork;
mod futex_wait;
mod futex_wake;
mod isatty;
mod open;
mod read;
mod seek;
mod stat;
mod tcb_set;
mod vm;
mod write;

use crate::Syscall;

pub(super) const SYSCALLS: [Syscall; 18] = [
    exit::SYSCALL,
    read::SYSCALL,
    write::SYSCALL,
    futex_wait::SYSCALL,
    futex_wake::SYSCALL,
    anon_allocate::SYSCALL,
    anon_free::SYSCALL,
    tcb_set::SYSCALL,
    clock_get::SYSCALL,
    vm::MAP_SYSCALL,
    vm::UNMAP_SYSCALL,
    close::SYSCALL,
    seek::SYSCALL,
    isatty::SYSCALL,
    open::SYSCALL,
    vm::PROTECT_SYSCALL,
    stat::SYSCALL,
    fork::SYSCALL,
];
