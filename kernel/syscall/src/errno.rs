/// The ABI errno values, translated from subsystem errors at this boundary.
///
/// Every variant is named for the error condition, never for the constant, and its number is the
/// one `abi-bits/errno.h` gives the name that condition has. A caller compares the value it receives
/// against that name, so the pair has to agree; a call site cannot check that, because it names the
/// variant rather than the number. `Pipe` once stood for both the descriptor that cannot be seeked
/// and the pipe whose peer is closed, and the callers that meant the second one reported the first
/// one's number.
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
    /// `ESPIPE`: the descriptor is one that cannot be seeked.
    InvalidSeek = 29,
    ReadOnly = 30,
    /// `EPIPE`: the read end of the pipe or socket this write addresses is closed.
    BrokenPipe = 32,
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

    /// The name `abi-bits/errno.h` gives each variant, and the number it gives that name.
    ///
    /// This is the header's table written out on the kernel side, which nothing else in the tree
    /// compares against: the discriminants above and the `#define`s there are two hand-kept copies
    /// of one numbering. The match is total, so a variant added above cannot reach a build without
    /// being given a number to be held to here.
    const fn abi_number(error: Errno) -> u64 {
        match error {
            Errno::Permission => 1,
            Errno::NotFound => 2,
            Errno::NoSuchProcess => 3,
            Errno::Interrupted => 4,
            Errno::Io => 5,
            Errno::TooBig => 7,
            Errno::ExecFormat => 8,
            Errno::BadFd => 9,
            Errno::Child => 10,
            Errno::Again => 11,
            Errno::NoMem => 12,
            Errno::Access => 13,
            Errno::Fault => 14,
            Errno::Busy => 16,
            Errno::AlreadyExists => 17,
            Errno::CrossDevice => 18,
            Errno::NotDirectory => 20,
            Errno::IsDirectory => 21,
            Errno::Invalid => 22,
            Errno::NotTty => 25,
            Errno::NoSpace => 28,
            Errno::InvalidSeek => 29,
            Errno::ReadOnly => 30,
            Errno::BrokenPipe => 32,
            Errno::Range => 34,
            Errno::NameTooLong => 36,
            Errno::NoSys => 38,
            Errno::NotEmpty => 39,
            Errno::Loop => 40,
            Errno::Overflow => 75,
            Errno::NotSocket => 88,
            Errno::NotSupported => 95,
            Errno::AddressInUse => 98,
            Errno::AlreadyConnected => 106,
            Errno::NotConnected => 107,
            Errno::ConnectionRefused => 111,
        }
    }

    kernel_test!("roxy-syscall::errno-values", numbers_match_the_roxy_abi, {
        for error in Errno::iter() {
            let expected = abi_number(error);

            // The error register carrying 0 is what tells a caller the call succeeded, so no
            // variant may be numbered 0.
            assert_ne!(expected, 0, "an errno is named for a number 0");
            assert_eq!(
                error.number(),
                expected,
                "an errno does not carry the number its name has"
            );
        }
    });
}
