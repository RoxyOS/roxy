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
        assert!(SyscallNumber::try_from(32).is_err());
    });
}
