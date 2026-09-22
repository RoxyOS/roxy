mod access;
mod anon_allocate;
mod anon_free;
mod chdir;
mod chmod;
mod clock;
mod close;
mod execve;
mod exit;
mod fchmod;
mod fd;
mod fork;
mod fs;
mod futex_wait;
mod futex_wake;
mod get_tid;
mod getcwd;
mod getegid;
mod geteuid;
mod getgid;
mod getpgid;
mod getpid;
mod getppid;
mod getuid;
mod ioctl;
mod iovec;
mod isatty;
mod open;
mod openpty;
mod pipe;
mod poll;
mod read;
mod seek;
mod setpgid;
mod setsid;
pub(crate) mod signal;
mod sigtimedwait;
mod sleep;
mod socket;
mod tcb_set;
mod tgkill;
mod thread_create;
mod thread_exit;
mod timer;
mod umask;
mod uname;
mod vm;
mod waitpid;
mod write;
mod writev;

use crate::Syscall;

pub(super) const SYSCALLS: [Syscall; 87] = [
    exit::SYSCALL,
    read::SYSCALL,
    write::SYSCALL,
    futex_wait::SYSCALL,
    futex_wake::SYSCALL,
    anon_allocate::SYSCALL,
    anon_free::SYSCALL,
    tcb_set::SYSCALL,
    clock::GET_SYSCALL,
    clock::GETRES_SYSCALL,
    vm::MAP_SYSCALL,
    vm::UNMAP_SYSCALL,
    close::SYSCALL,
    seek::SYSCALL,
    isatty::SYSCALL,
    open::SYSCALL,
    vm::PROTECT_SYSCALL,
    fs::STAT_SYSCALL,
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
    fs::OPEN_DIR_SYSCALL,
    fs::READ_ENTRIES_SYSCALL,
    chdir::SYSCALL,
    ioctl::SYSCALL,
    getcwd::SYSCALL,
    poll::POLL_SYSCALL,
    sleep::SYSCALL,
    signal::SEND_SYSCALL,
    poll::PPOLL_SYSCALL,
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
    socket::RECVMSG_SYSCALL,
    socket::SENDMSG_SYSCALL,
    socket::SOCKETPAIR_SYSCALL,
    signal::SIGRETURN_SYSCALL,
    pipe::SYSCALL,
    fd::DUP_ONTO_SYSCALL,
    fd::DUP_SYSCALL,
    umask::SYSCALL,
    chmod::SYSCALL,
    fchmod::SYSCALL,
    access::SYSCALL,
    setpgid::SYSCALL,
    getpgid::SYSCALL,
    setsid::SYSCALL,
    writev::SYSCALL,
    timer::CREATE_SYSCALL,
    timer::SETTIME_SYSCALL,
    timer::GETTIME_SYSCALL,
    timer::GETOVERRUN_SYSCALL,
    timer::DELETE_SYSCALL,
    thread_create::SYSCALL,
    thread_exit::SYSCALL,
    get_tid::SYSCALL,
    sigtimedwait::SYSCALL,
    tgkill::SYSCALL,
    openpty::SYSCALL,
    fd::GET_DESCRIPTOR_FLAGS_SYSCALL,
    fd::SET_DESCRIPTOR_FLAGS_SYSCALL,
    fd::GET_STATUS_FLAGS_SYSCALL,
    fd::SET_STATUS_FLAGS_SYSCALL,
];
