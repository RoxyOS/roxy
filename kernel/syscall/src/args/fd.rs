use roxy_fd::Fd;

use super::SyscallArg;
use crate::errno::Errno;

impl SyscallArg for Fd {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        u32::try_from(raw).map(Fd::new).map_err(|_| error)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::Fd;
    use roxy_test::kernel_test;

    use super::SyscallArg;
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::fd-argument-range", rejects_out_of_range, {
        assert!(
            Fd::parse(u64::from(u32::MAX) + 1, Errno::BadFd)
                .is_err_and(|error| error == Errno::BadFd)
        );
    });
}
