#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Errno {
    NotFound = 2,
    Io = 5,
    BadFd = 9,
    Again = 11,
    NoMem = 12,
    Access = 13,
    Fault = 14,
    Busy = 16,
    AlreadyExists = 17,
    CrossDevice = 18,
    NotDirectory = 20,
    IsDirectory = 21,
    Invalid = 22,
    NotTty = 25,
    NoSpace = 28,
    Pipe = 29,
    ReadOnly = 30,
    NameTooLong = 36,
    NoSys = 38,
    Overflow = 75,
    NotSupported = 95,
}

impl Errno {
    #[must_use]
    pub(crate) const fn number(self) -> u64 {
        self as u64
    }

    #[must_use]
    pub(crate) const fn encode(self) -> u64 {
        (-(self as i64)).cast_unsigned()
    }
}

#[cfg(test)]
mod tests {
    use super::Errno;

    #[test]
    fn errno_encodes_as_negative_return() {
        assert_eq!(Errno::NoSys.encode().cast_signed(), -38);
    }
}
