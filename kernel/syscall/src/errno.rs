#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Errno {
    Again = 11,
    NoSys = 38,
    BadFd = 9,
    Fault = 14,
    Io = 5,
    Invalid = 22,
    NoMem = 12,
    Overflow = 75,
    Pipe = 29,
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
