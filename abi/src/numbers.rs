use core::mem::{align_of, size_of};

#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyscallNumber {
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

const _: () = {
    assert!(size_of::<SyscallNumber>() == 8);
    assert!(align_of::<SyscallNumber>() == 8);
};

#[cfg(test)]
mod tests {
    use super::SyscallNumber;

    #[test]
    fn syscall_number_rejects_unknown_value() {
        assert_eq!(SyscallNumber::try_from(0), Ok(SyscallNumber::Exit));
        assert!(SyscallNumber::try_from(1).is_err());
    }
}
