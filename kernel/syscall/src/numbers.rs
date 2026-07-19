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
        assert!(SyscallNumber::try_from(19).is_err());
    });
}
