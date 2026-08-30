mod access;
mod anon_allocate;
mod anon_free;
mod chdir;
mod chmod;
mod clock_get;
mod close;
mod dup2;
mod execve;
mod exit;
mod fchmod;
mod fcntl;
mod fork;
mod fs;
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
mod pipe;
mod poll;
mod read;
mod read_entries;
mod seek;
pub(crate) mod signal;
mod sleep;
mod socket;
mod socketpair;
mod stat;
mod tcb_set;
mod umask;
mod uname;
mod vm;
mod waitpid;
mod write;

use crate::Syscall;

pub(super) const SYSCALLS: [Syscall; 66] = [
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
    signal::MASK_SYSCALL,
    signal::ACTION_SYSCALL,
    open_dir::SYSCALL,
    read_entries::SYSCALL,
    chdir::SYSCALL,
    ioctl::SYSCALL,
    getcwd::SYSCALL,
    poll::POLL_SYSCALL,
    sleep::SYSCALL,
    signal::SEND_SYSCALL,
    poll::PPOLL_SYSCALL,
    poll::PSELECT_SYSCALL,
    uname::SYSCALL,
    fs::MKDIRAT_SYSCALL,
    fs::UNLINKAT_SYSCALL,
    fs::READLINKAT_SYSCALL,
    fs::LINKAT_SYSCALL,
    fs::SYMLINKAT_SYSCALL,
    fs::RENAMEAT_SYSCALL,
    fs::SYNC_SYSCALL,
    fs::FSYNC_SYSCALL,
    fs::FTRUNCATE_SYSCALL,
    socket::SOCKET_SYSCALL,
    socket::BIND_SYSCALL,
    socket::LISTEN_SYSCALL,
    socket::ACCEPT_SYSCALL,
    socket::CONNECT_SYSCALL,
    socket::SHUTDOWN_SYSCALL,
    socket::GETSOCKNAME_SYSCALL,
    socket::GETPEERNAME_SYSCALL,
    socket::GETSOCKOPT_SYSCALL,
    socketpair::SYSCALL,
    signal::SIGRETURN_SYSCALL,
    pipe::SYSCALL,
    dup2::SYSCALL,
    fcntl::SYSCALL,
    umask::SYSCALL,
    chmod::SYSCALL,
    fchmod::SYSCALL,
    access::SYSCALL,
];
