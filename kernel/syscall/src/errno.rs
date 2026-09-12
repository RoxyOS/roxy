#[repr(u64)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, strum::EnumIter)]
pub(crate) enum Errno {
    Permission = 1,
    NotFound = 2,
    NoSuchProcess = 3,
    Interrupted = 4,
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
    NotEmpty = 39,
    Loop = 40,
    Overflow = 75,
    NotSocket = 88,
    NotSupported = 95,
    AddressInUse = 98,
    AlreadyConnected = 106,
    NotConnected = 107,
    ConnectionRefused = 111,
}

impl Errno {
    /// The value the personality writes to the return path's error register, or `None` on success.
    ///
    /// Zero is reserved for success, so every variant is numbered above it.
    #[must_use]
    pub(crate) const fn number(self) -> u64 {
        self as u64
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;
    use strum::IntoEnumIterator;

    use super::Errno;

    kernel_test!("roxy-syscall::errno-values", keeps_zero_free_for_success, {
        // The error register carrying 0 is what tells a caller that the call succeeded, so no
        // variant may be numbered 0.
        for error in Errno::iter() {
            assert_ne!(error.number(), 0, "an errno is numbered 0");
        }

        assert_eq!(Errno::Permission.number(), 1);
    });
}
