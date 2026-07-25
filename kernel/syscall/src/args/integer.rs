use super::{RawSyscallArg, SyscallArg};
use crate::errno::Errno;

impl RawSyscallArg for u64 {
    fn parse(raw: u64) -> Self {
        raw
    }
}

impl RawSyscallArg for i64 {
    fn parse(raw: u64) -> Self {
        raw.cast_signed()
    }
}

impl SyscallArg for usize {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        usize::try_from(raw).map_err(|_| error)
    }
}

impl SyscallArg for u32 {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        u32::try_from(raw).map_err(|_| error)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::SyscallArg;
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::integer-argument-error", preserves_error, {
        assert_eq!(u32::parse(u64::MAX, Errno::Range), Err(Errno::Range));
    });
}
