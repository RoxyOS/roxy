#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Errno {
    NotFound = 2,
    NoSuchProcess = 3,
    TooBig = 7,
    ExecFormat = 8,
    Io = 5,
    BadFd = 9,
    Child = 10,
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
    Range = 34,
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

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::Errno;

    kernel_test!("roxy-syscall::errno-encoding", encodes_negative_return, {
        assert_eq!(Errno::NoSys.encode().cast_signed(), -38);
    });
}
