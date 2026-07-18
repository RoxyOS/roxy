#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SyscallNumber {
    Exit = 0,
}

impl TryFrom<u64> for SyscallNumber {
    type Error = ();

    fn try_from(number: u64) -> Result<Self, Self::Error> {
        match number {
            0 => Ok(Self::Exit),
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
        assert!(SyscallNumber::try_from(1).is_err());
    }
}
