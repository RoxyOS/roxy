/// Base of the Roxy syscall space: a syscall number is this base plus the syscall's index.
///
/// It sits above every number another personality uses for the syscalls this kernel provides —
/// Linux's `x86_64` table ends in the low hundreds and the BSDs' numbers are no larger — so a
/// number below the base is never ours, and dispatch can report a caller that issued a foreign
/// number rather than misreading it as a syscall of its own. It also leaves the low range free for
/// a Linux-compatible personality to serve unshifted. Userspace spells it `ROXY_SYS_BASE`.
pub(crate) const SYSCALL_BASE: u64 = 0x1000;

#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SyscallNumber {
    Exit = SYSCALL_BASE,
    Read = SYSCALL_BASE + 1,
    Write = SYSCALL_BASE + 2,
    FutexWait = SYSCALL_BASE + 3,
    FutexWake = SYSCALL_BASE + 4,
    AnonAllocate = SYSCALL_BASE + 5,
    AnonFree = SYSCALL_BASE + 6,
    TcbSet = SYSCALL_BASE + 7,
    ClockGet = SYSCALL_BASE + 8,
    VmMap = SYSCALL_BASE + 9,
    VmUnmap = SYSCALL_BASE + 10,
    Close = SYSCALL_BASE + 11,
    Seek = SYSCALL_BASE + 12,
    Isatty = SYSCALL_BASE + 13,
    Open = SYSCALL_BASE + 14,
    VmProtect = SYSCALL_BASE + 15,
    Stat = SYSCALL_BASE + 16,
    Fork = SYSCALL_BASE + 17,
    Execve = SYSCALL_BASE + 18,
    Getpid = SYSCALL_BASE + 19,
    Getppid = SYSCALL_BASE + 20,
    Geteuid = SYSCALL_BASE + 21,
    Getuid = SYSCALL_BASE + 22,
    Getgid = SYSCALL_BASE + 23,
    Getegid = SYSCALL_BASE + 24,
    Waitpid = SYSCALL_BASE + 25,
    Sigprocmask = SYSCALL_BASE + 26,
    Sigaction = SYSCALL_BASE + 27,
    OpenDir = SYSCALL_BASE + 28,
    ReadEntries = SYSCALL_BASE + 29,
    Chdir = SYSCALL_BASE + 30,
    Ioctl = SYSCALL_BASE + 31,
    Getcwd = SYSCALL_BASE + 32,
    Poll = SYSCALL_BASE + 33,
    Sleep = SYSCALL_BASE + 34,
    SendSignal = SYSCALL_BASE + 35,
    Ppoll = SYSCALL_BASE + 36,
    Uname = SYSCALL_BASE + 37,
    Mkdirat = SYSCALL_BASE + 38,
    Unlinkat = SYSCALL_BASE + 39,
    Readlinkat = SYSCALL_BASE + 40,
    Linkat = SYSCALL_BASE + 41,
    Symlinkat = SYSCALL_BASE + 42,
    Renameat = SYSCALL_BASE + 43,
    Sync = SYSCALL_BASE + 44,
    Fsync = SYSCALL_BASE + 45,
    Ftruncate = SYSCALL_BASE + 46,
    Socketpair = SYSCALL_BASE + 47,
    Socket = SYSCALL_BASE + 48,
    Bind = SYSCALL_BASE + 49,
    Listen = SYSCALL_BASE + 50,
    Accept = SYSCALL_BASE + 51,
    Connect = SYSCALL_BASE + 52,
    Sigreturn = SYSCALL_BASE + 53,
    Pipe = SYSCALL_BASE + 54,
    Dup2 = SYSCALL_BASE + 55,
    Fcntl = SYSCALL_BASE + 56,
    Umask = SYSCALL_BASE + 57,
    Chmod = SYSCALL_BASE + 58,
    Fchmod = SYSCALL_BASE + 59,
    Sockname = SYSCALL_BASE + 60,
    Peername = SYSCALL_BASE + 61,
    Shutdown = SYSCALL_BASE + 62,
    GetSockopt = SYSCALL_BASE + 63,
    Access = SYSCALL_BASE + 64,
    RecvMsg = SYSCALL_BASE + 65,
    SendMsg = SYSCALL_BASE + 66,
    SetPgid = SYSCALL_BASE + 67,
    GetPgid = SYSCALL_BASE + 68,
    SetSid = SYSCALL_BASE + 69,
    Writev = SYSCALL_BASE + 70,
    Ttyname = SYSCALL_BASE + 71,
    TimerCreate = SYSCALL_BASE + 72,
    TimerSettime = SYSCALL_BASE + 73,
    TimerGettime = SYSCALL_BASE + 74,
    TimerGetoverrun = SYSCALL_BASE + 75,
    TimerDelete = SYSCALL_BASE + 76,
    ThreadCreate = SYSCALL_BASE + 77,
    ThreadExit = SYSCALL_BASE + 78,
    GetTid = SYSCALL_BASE + 79,
    SigtimedWait = SYSCALL_BASE + 80,
    Tgkill = SYSCALL_BASE + 81,
    ClockGetres = SYSCALL_BASE + 82,
    Openpty = SYSCALL_BASE + 83,
}

impl TryFrom<u64> for SyscallNumber {
    type Error = ();

    fn try_from(number: u64) -> Result<Self, Self::Error> {
        let Some(index) = number.checked_sub(SYSCALL_BASE) else {
            return Err(());
        };

        match index {
            0 => Ok(Self::Exit),
            1 => Ok(Self::Read),
            2 => Ok(Self::Write),
            3 => Ok(Self::FutexWait),
            4 => Ok(Self::FutexWake),
            5 => Ok(Self::AnonAllocate),
            6 => Ok(Self::AnonFree),
            7 => Ok(Self::TcbSet),
            8 => Ok(Self::ClockGet),
            9 => Ok(Self::VmMap),
            10 => Ok(Self::VmUnmap),
            11 => Ok(Self::Close),
            12 => Ok(Self::Seek),
            13 => Ok(Self::Isatty),
            14 => Ok(Self::Open),
            15 => Ok(Self::VmProtect),
            16 => Ok(Self::Stat),
            17 => Ok(Self::Fork),
            18 => Ok(Self::Execve),
            19 => Ok(Self::Getpid),
            20 => Ok(Self::Getppid),
            21 => Ok(Self::Geteuid),
            22 => Ok(Self::Getuid),
            23 => Ok(Self::Getgid),
            24 => Ok(Self::Getegid),
            25 => Ok(Self::Waitpid),
            26 => Ok(Self::Sigprocmask),
            27 => Ok(Self::Sigaction),
            28 => Ok(Self::OpenDir),
            29 => Ok(Self::ReadEntries),
            30 => Ok(Self::Chdir),
            31 => Ok(Self::Ioctl),
            32 => Ok(Self::Getcwd),
            33 => Ok(Self::Poll),
            34 => Ok(Self::Sleep),
            35 => Ok(Self::SendSignal),
            36 => Ok(Self::Ppoll),
            37 => Ok(Self::Uname),
            38 => Ok(Self::Mkdirat),
            39 => Ok(Self::Unlinkat),
            40 => Ok(Self::Readlinkat),
            41 => Ok(Self::Linkat),
            42 => Ok(Self::Symlinkat),
            43 => Ok(Self::Renameat),
            44 => Ok(Self::Sync),
            45 => Ok(Self::Fsync),
            46 => Ok(Self::Ftruncate),
            47 => Ok(Self::Socketpair),
            48 => Ok(Self::Socket),
            49 => Ok(Self::Bind),
            50 => Ok(Self::Listen),
            51 => Ok(Self::Accept),
            52 => Ok(Self::Connect),
            53 => Ok(Self::Sigreturn),
            54 => Ok(Self::Pipe),
            55 => Ok(Self::Dup2),
            56 => Ok(Self::Fcntl),
            57 => Ok(Self::Umask),
            58 => Ok(Self::Chmod),
            59 => Ok(Self::Fchmod),
            60 => Ok(Self::Sockname),
            61 => Ok(Self::Peername),
            62 => Ok(Self::Shutdown),
            63 => Ok(Self::GetSockopt),
            64 => Ok(Self::Access),
            65 => Ok(Self::RecvMsg),
            66 => Ok(Self::SendMsg),
            67 => Ok(Self::SetPgid),
            68 => Ok(Self::GetPgid),
            69 => Ok(Self::SetSid),
            70 => Ok(Self::Writev),
            71 => Ok(Self::Ttyname),
            72 => Ok(Self::TimerCreate),
            73 => Ok(Self::TimerSettime),
            74 => Ok(Self::TimerGettime),
            75 => Ok(Self::TimerGetoverrun),
            76 => Ok(Self::TimerDelete),
            77 => Ok(Self::ThreadCreate),
            78 => Ok(Self::ThreadExit),
            79 => Ok(Self::GetTid),
            80 => Ok(Self::SigtimedWait),
            81 => Ok(Self::Tgkill),
            82 => Ok(Self::ClockGetres),
            83 => Ok(Self::Openpty),
            _ => Err(()),
        }
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::{SYSCALL_BASE, SyscallNumber};
    use roxy_test::kernel_test;

    kernel_test!("roxy-syscall::number-conversion", number_conversion, {
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE),
            Ok(SyscallNumber::Exit)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 1),
            Ok(SyscallNumber::Read)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 2),
            Ok(SyscallNumber::Write)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 3),
            Ok(SyscallNumber::FutexWait)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 4),
            Ok(SyscallNumber::FutexWake)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 5),
            Ok(SyscallNumber::AnonAllocate)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 6),
            Ok(SyscallNumber::AnonFree)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 7),
            Ok(SyscallNumber::TcbSet)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 8),
            Ok(SyscallNumber::ClockGet)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 9),
            Ok(SyscallNumber::VmMap)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 10),
            Ok(SyscallNumber::VmUnmap)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 11),
            Ok(SyscallNumber::Close)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 12),
            Ok(SyscallNumber::Seek)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 13),
            Ok(SyscallNumber::Isatty)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 14),
            Ok(SyscallNumber::Open)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 15),
            Ok(SyscallNumber::VmProtect)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 16),
            Ok(SyscallNumber::Stat)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 17),
            Ok(SyscallNumber::Fork)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 18),
            Ok(SyscallNumber::Execve)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 19),
            Ok(SyscallNumber::Getpid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 20),
            Ok(SyscallNumber::Getppid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 21),
            Ok(SyscallNumber::Geteuid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 22),
            Ok(SyscallNumber::Getuid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 23),
            Ok(SyscallNumber::Getgid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 24),
            Ok(SyscallNumber::Getegid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 25),
            Ok(SyscallNumber::Waitpid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 26),
            Ok(SyscallNumber::Sigprocmask)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 27),
            Ok(SyscallNumber::Sigaction)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 28),
            Ok(SyscallNumber::OpenDir)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 29),
            Ok(SyscallNumber::ReadEntries)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 30),
            Ok(SyscallNumber::Chdir)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 31),
            Ok(SyscallNumber::Ioctl)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 32),
            Ok(SyscallNumber::Getcwd)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 33),
            Ok(SyscallNumber::Poll)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 34),
            Ok(SyscallNumber::Sleep)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 35),
            Ok(SyscallNumber::SendSignal)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 36),
            Ok(SyscallNumber::Ppoll)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 37),
            Ok(SyscallNumber::Uname)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 38),
            Ok(SyscallNumber::Mkdirat)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 39),
            Ok(SyscallNumber::Unlinkat)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 40),
            Ok(SyscallNumber::Readlinkat)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 41),
            Ok(SyscallNumber::Linkat)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 42),
            Ok(SyscallNumber::Symlinkat)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 43),
            Ok(SyscallNumber::Renameat)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 44),
            Ok(SyscallNumber::Sync)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 45),
            Ok(SyscallNumber::Fsync)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 46),
            Ok(SyscallNumber::Ftruncate)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 47),
            Ok(SyscallNumber::Socketpair)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 48),
            Ok(SyscallNumber::Socket)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 49),
            Ok(SyscallNumber::Bind)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 50),
            Ok(SyscallNumber::Listen)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 51),
            Ok(SyscallNumber::Accept)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 52),
            Ok(SyscallNumber::Connect)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 53),
            Ok(SyscallNumber::Sigreturn)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 54),
            Ok(SyscallNumber::Pipe)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 55),
            Ok(SyscallNumber::Dup2)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 56),
            Ok(SyscallNumber::Fcntl)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 57),
            Ok(SyscallNumber::Umask)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 58),
            Ok(SyscallNumber::Chmod)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 59),
            Ok(SyscallNumber::Fchmod)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 60),
            Ok(SyscallNumber::Sockname)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 61),
            Ok(SyscallNumber::Peername)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 62),
            Ok(SyscallNumber::Shutdown)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 63),
            Ok(SyscallNumber::GetSockopt)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 64),
            Ok(SyscallNumber::Access)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 65),
            Ok(SyscallNumber::RecvMsg)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 66),
            Ok(SyscallNumber::SendMsg)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 67),
            Ok(SyscallNumber::SetPgid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 68),
            Ok(SyscallNumber::GetPgid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 69),
            Ok(SyscallNumber::SetSid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 70),
            Ok(SyscallNumber::Writev)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 71),
            Ok(SyscallNumber::Ttyname)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 72),
            Ok(SyscallNumber::TimerCreate)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 73),
            Ok(SyscallNumber::TimerSettime)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 74),
            Ok(SyscallNumber::TimerGettime)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 75),
            Ok(SyscallNumber::TimerGetoverrun)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 76),
            Ok(SyscallNumber::TimerDelete)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 77),
            Ok(SyscallNumber::ThreadCreate)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 78),
            Ok(SyscallNumber::ThreadExit)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 79),
            Ok(SyscallNumber::GetTid)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 80),
            Ok(SyscallNumber::SigtimedWait)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 81),
            Ok(SyscallNumber::Tgkill)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 82),
            Ok(SyscallNumber::ClockGetres)
        );
        assert_eq!(
            SyscallNumber::try_from(SYSCALL_BASE + 83),
            Ok(SyscallNumber::Openpty)
        );
        assert!(SyscallNumber::try_from(SYSCALL_BASE + 84).is_err());

        // Below the base is another personality's numbering, which the dispatcher reports as
        // foreign. Linux x86_64's `read` is 0, `write` is 1, and `exit_group` is 231.
        for foreign in [0, 1, 2, 60, 231, SYSCALL_BASE - 1] {
            assert!(SyscallNumber::try_from(foreign).is_err());
        }
    });
}
