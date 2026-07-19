mod anon_allocate;
mod anon_free;
mod clock_get;
mod close;
mod exit;
mod futex_wait;
mod futex_wake;
mod read;
mod seek;
mod tcb_set;
mod vm_map;
mod vm_unmap;
mod write;

use crate::Syscall;

pub(super) const SYSCALLS: [Syscall; 13] = [
    exit::SYSCALL,
    read::SYSCALL,
    write::SYSCALL,
    futex_wait::SYSCALL,
    futex_wake::SYSCALL,
    anon_allocate::SYSCALL,
    anon_free::SYSCALL,
    tcb_set::SYSCALL,
    clock_get::SYSCALL,
    vm_map::SYSCALL,
    vm_unmap::SYSCALL,
    close::SYSCALL,
    seek::SYSCALL,
];
