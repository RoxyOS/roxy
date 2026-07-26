use roxy_memory::UserAddress;

use super::SyscallArg;
use crate::errno::Errno;

impl SyscallArg for UserAddress {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        UserAddress::new(raw).ok_or(error)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_memory::UserAddress;
    use roxy_test::kernel_test;

    use super::SyscallArg;
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::address-argument-errors", preserves_errors, {
        assert_eq!(UserAddress::parse(0, Errno::Fault), Err(Errno::Fault));
        assert_eq!(UserAddress::parse(0, Errno::Invalid), Err(Errno::Invalid));
    });
}
