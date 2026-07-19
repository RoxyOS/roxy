#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SyscallNumber {
    Exit = 0,
    Read = 1,
    Write = 2,
    FutexWait = 3,
    FutexWake = 4,
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
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SyscallNumber;

    #[test]
    fn syscall_number_rejects_unknown_value() {
        assert_eq!(SyscallNumber::try_from(0), Ok(SyscallNumber::Exit));
        assert_eq!(SyscallNumber::try_from(1), Ok(SyscallNumber::Read));
        assert_eq!(SyscallNumber::try_from(2), Ok(SyscallNumber::Write));
        assert_eq!(SyscallNumber::try_from(3), Ok(SyscallNumber::FutexWait));
        assert_eq!(SyscallNumber::try_from(4), Ok(SyscallNumber::FutexWake));
        assert!(SyscallNumber::try_from(5).is_err());
    }
}
