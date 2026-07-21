use crate::{Syscall, SyscallResult, numbers::SyscallNumber};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Getgid, handle);

#[allow(clippy::unnecessary_wraps)]
fn handle(_arguments: [u64; 6]) -> SyscallResult {
    Ok(0)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::handle;

    kernel_test!("roxy-syscall::getgid", getgid, {
        assert_eq!(handle([0; 6]), Ok(0));
    });
}
