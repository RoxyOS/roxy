#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SyscallNumber {
    Exit = 0,
    Read = 1,
    Write = 2,
    FutexWait = 3,
    FutexWake = 4,
    AnonAllocate = 5,
    AnonFree = 6,
    TcbSet = 7,
    ClockGet = 8,
    VmMap = 9,
    VmUnmap = 10,
    Close = 11,
    Seek = 12,
    Isatty = 13,
    Open = 14,
    VmProtect = 15,
    Stat = 16,
    Fork = 17,
    Execve = 18,
    Getpid = 19,
    Getppid = 20,
    Geteuid = 21,
    Getuid = 22,
    Getgid = 23,
    Getegid = 24,
    Waitpid = 25,
    Sigprocmask = 26,
    Sigaction = 27,
    OpenDir = 28,
    ReadEntries = 29,
    Chdir = 30,
    Ioctl = 31,
    Getcwd = 32,
    Poll = 33,
    Sleep = 34,
    SendSignal = 35,
    Ppoll = 36,
    Pselect = 37,
    Uname = 38,
    Mkdirat = 39,
    Unlinkat = 40,
    Readlinkat = 41,
    Linkat = 42,
    Symlinkat = 43,
    Renameat = 44,
    Sync = 45,
    Fsync = 46,
    Ftruncate = 47,
    Socketpair = 48,
    Socket = 49,
    Bind = 50,
    Listen = 51,
    Accept = 52,
    Connect = 53,
    Sigreturn = 54,
    Pipe = 55,
    Dup2 = 56,
    Fcntl = 57,
    Umask = 58,
    Chmod = 59,
    Fchmod = 60,
    Sockname = 61,
    Peername = 62,
    Shutdown = 63,
    GetSockopt = 64,
    Access = 65,
    RecvMsg = 66,
    SendMsg = 67,
    SetPgid = 68,
    GetPgid = 69,
    SetSid = 70,
    Writev = 71,
}

impl TryFrom<u64> for SyscallNumber {
    type Error = ();

    fn try_from(number: u64) -> Result<Self, Self::Error> {
        match number {
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
            37 => Ok(Self::Pselect),
            38 => Ok(Self::Uname),
            39 => Ok(Self::Mkdirat),
            40 => Ok(Self::Unlinkat),
            41 => Ok(Self::Readlinkat),
            42 => Ok(Self::Linkat),
            43 => Ok(Self::Symlinkat),
            44 => Ok(Self::Renameat),
            45 => Ok(Self::Sync),
            46 => Ok(Self::Fsync),
            47 => Ok(Self::Ftruncate),
            48 => Ok(Self::Socketpair),
            49 => Ok(Self::Socket),
            50 => Ok(Self::Bind),
            51 => Ok(Self::Listen),
            52 => Ok(Self::Accept),
            53 => Ok(Self::Connect),
            54 => Ok(Self::Sigreturn),
            55 => Ok(Self::Pipe),
            56 => Ok(Self::Dup2),
            57 => Ok(Self::Fcntl),
            58 => Ok(Self::Umask),
            59 => Ok(Self::Chmod),
            60 => Ok(Self::Fchmod),
            61 => Ok(Self::Sockname),
            62 => Ok(Self::Peername),
            63 => Ok(Self::Shutdown),
            64 => Ok(Self::GetSockopt),
            65 => Ok(Self::Access),
            66 => Ok(Self::RecvMsg),
            67 => Ok(Self::SendMsg),
            68 => Ok(Self::SetPgid),
            69 => Ok(Self::GetPgid),
            70 => Ok(Self::SetSid),
            71 => Ok(Self::Writev),
            _ => Err(()),
        }
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use super::SyscallNumber;
    use roxy_test::kernel_test;

    kernel_test!("roxy-syscall::number-conversion", number_conversion, {
        assert_eq!(SyscallNumber::try_from(0), Ok(SyscallNumber::Exit));
        assert_eq!(SyscallNumber::try_from(1), Ok(SyscallNumber::Read));
        assert_eq!(SyscallNumber::try_from(2), Ok(SyscallNumber::Write));
        assert_eq!(SyscallNumber::try_from(3), Ok(SyscallNumber::FutexWait));
        assert_eq!(SyscallNumber::try_from(4), Ok(SyscallNumber::FutexWake));
        assert_eq!(SyscallNumber::try_from(5), Ok(SyscallNumber::AnonAllocate));
        assert_eq!(SyscallNumber::try_from(6), Ok(SyscallNumber::AnonFree));
        assert_eq!(SyscallNumber::try_from(7), Ok(SyscallNumber::TcbSet));
        assert_eq!(SyscallNumber::try_from(8), Ok(SyscallNumber::ClockGet));
        assert_eq!(SyscallNumber::try_from(9), Ok(SyscallNumber::VmMap));
        assert_eq!(SyscallNumber::try_from(10), Ok(SyscallNumber::VmUnmap));
        assert_eq!(SyscallNumber::try_from(11), Ok(SyscallNumber::Close));
        assert_eq!(SyscallNumber::try_from(12), Ok(SyscallNumber::Seek));
        assert_eq!(SyscallNumber::try_from(13), Ok(SyscallNumber::Isatty));
        assert_eq!(SyscallNumber::try_from(14), Ok(SyscallNumber::Open));
        assert_eq!(SyscallNumber::try_from(15), Ok(SyscallNumber::VmProtect));
        assert_eq!(SyscallNumber::try_from(16), Ok(SyscallNumber::Stat));
        assert_eq!(SyscallNumber::try_from(17), Ok(SyscallNumber::Fork));
        assert_eq!(SyscallNumber::try_from(18), Ok(SyscallNumber::Execve));
        assert_eq!(SyscallNumber::try_from(19), Ok(SyscallNumber::Getpid));
        assert_eq!(SyscallNumber::try_from(20), Ok(SyscallNumber::Getppid));
        assert_eq!(SyscallNumber::try_from(21), Ok(SyscallNumber::Geteuid));
        assert_eq!(SyscallNumber::try_from(22), Ok(SyscallNumber::Getuid));
        assert_eq!(SyscallNumber::try_from(23), Ok(SyscallNumber::Getgid));
        assert_eq!(SyscallNumber::try_from(24), Ok(SyscallNumber::Getegid));
        assert_eq!(SyscallNumber::try_from(25), Ok(SyscallNumber::Waitpid));
        assert_eq!(SyscallNumber::try_from(26), Ok(SyscallNumber::Sigprocmask));
        assert_eq!(SyscallNumber::try_from(27), Ok(SyscallNumber::Sigaction));
        assert_eq!(SyscallNumber::try_from(28), Ok(SyscallNumber::OpenDir));
        assert_eq!(SyscallNumber::try_from(29), Ok(SyscallNumber::ReadEntries));
        assert_eq!(SyscallNumber::try_from(30), Ok(SyscallNumber::Chdir));
        assert_eq!(SyscallNumber::try_from(31), Ok(SyscallNumber::Ioctl));
        assert_eq!(SyscallNumber::try_from(32), Ok(SyscallNumber::Getcwd));
        assert_eq!(SyscallNumber::try_from(33), Ok(SyscallNumber::Poll));
        assert_eq!(SyscallNumber::try_from(34), Ok(SyscallNumber::Sleep));
        assert_eq!(SyscallNumber::try_from(35), Ok(SyscallNumber::SendSignal));
        assert_eq!(SyscallNumber::try_from(36), Ok(SyscallNumber::Ppoll));
        assert_eq!(SyscallNumber::try_from(37), Ok(SyscallNumber::Pselect));
        assert_eq!(SyscallNumber::try_from(38), Ok(SyscallNumber::Uname));
        assert_eq!(SyscallNumber::try_from(39), Ok(SyscallNumber::Mkdirat));
        assert_eq!(SyscallNumber::try_from(40), Ok(SyscallNumber::Unlinkat));
        assert_eq!(SyscallNumber::try_from(41), Ok(SyscallNumber::Readlinkat));
        assert_eq!(SyscallNumber::try_from(42), Ok(SyscallNumber::Linkat));
        assert_eq!(SyscallNumber::try_from(43), Ok(SyscallNumber::Symlinkat));
        assert_eq!(SyscallNumber::try_from(44), Ok(SyscallNumber::Renameat));
        assert_eq!(SyscallNumber::try_from(45), Ok(SyscallNumber::Sync));
        assert_eq!(SyscallNumber::try_from(46), Ok(SyscallNumber::Fsync));
        assert_eq!(SyscallNumber::try_from(47), Ok(SyscallNumber::Ftruncate));
        assert_eq!(SyscallNumber::try_from(48), Ok(SyscallNumber::Socketpair));
        assert_eq!(SyscallNumber::try_from(49), Ok(SyscallNumber::Socket));
        assert_eq!(SyscallNumber::try_from(50), Ok(SyscallNumber::Bind));
        assert_eq!(SyscallNumber::try_from(51), Ok(SyscallNumber::Listen));
        assert_eq!(SyscallNumber::try_from(52), Ok(SyscallNumber::Accept));
        assert_eq!(SyscallNumber::try_from(53), Ok(SyscallNumber::Connect));
        assert_eq!(SyscallNumber::try_from(54), Ok(SyscallNumber::Sigreturn));
        assert_eq!(SyscallNumber::try_from(55), Ok(SyscallNumber::Pipe));
        assert_eq!(SyscallNumber::try_from(56), Ok(SyscallNumber::Dup2));
        assert_eq!(SyscallNumber::try_from(57), Ok(SyscallNumber::Fcntl));
        assert_eq!(SyscallNumber::try_from(58), Ok(SyscallNumber::Umask));
        assert_eq!(SyscallNumber::try_from(59), Ok(SyscallNumber::Chmod));
        assert_eq!(SyscallNumber::try_from(60), Ok(SyscallNumber::Fchmod));
        assert_eq!(SyscallNumber::try_from(61), Ok(SyscallNumber::Sockname));
        assert_eq!(SyscallNumber::try_from(62), Ok(SyscallNumber::Peername));
        assert_eq!(SyscallNumber::try_from(63), Ok(SyscallNumber::Shutdown));
        assert_eq!(SyscallNumber::try_from(64), Ok(SyscallNumber::GetSockopt));
        assert_eq!(SyscallNumber::try_from(65), Ok(SyscallNumber::Access));
        assert_eq!(SyscallNumber::try_from(66), Ok(SyscallNumber::RecvMsg));
        assert_eq!(SyscallNumber::try_from(67), Ok(SyscallNumber::SendMsg));
        assert_eq!(SyscallNumber::try_from(68), Ok(SyscallNumber::SetPgid));
        assert_eq!(SyscallNumber::try_from(69), Ok(SyscallNumber::GetPgid));
        assert_eq!(SyscallNumber::try_from(70), Ok(SyscallNumber::SetSid));
        assert_eq!(SyscallNumber::try_from(71), Ok(SyscallNumber::Writev));
        assert!(SyscallNumber::try_from(72).is_err());
    });
}
