mod anon_allocate;
mod anon_free;
mod chdir;
mod clock_get;
mod close;
mod execve;
mod exit;
mod fork;
mod futex_wait;
mod futex_wake;
mod getcwd;
mod getegid;
mod geteuid;
mod getgid;
mod getpid;
mod getppid;
mod getuid;
mod ioctl;
mod isatty;
mod open;
mod open_dir;
mod read;
mod read_entries;
mod seek;
mod sigaction;
mod sigprocmask;
mod stat;
mod tcb_set;
mod vm;
mod waitpid;
mod write;

use crate::Syscall;

pub(super) const SYSCALLS: [Syscall; 33] = [
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
    execve::SYSCALL,
    getpid::SYSCALL,
    getppid::SYSCALL,
    geteuid::SYSCALL,
    getuid::SYSCALL,
    getgid::SYSCALL,
    getegid::SYSCALL,
    waitpid::SYSCALL,
    sigprocmask::SYSCALL,
    sigaction::SYSCALL,
    open_dir::SYSCALL,
    read_entries::SYSCALL,
    chdir::SYSCALL,
    ioctl::SYSCALL,
    getcwd::SYSCALL,
];
