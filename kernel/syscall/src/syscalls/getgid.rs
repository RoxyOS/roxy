use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Getgid, handle());

#[allow(clippy::unnecessary_wraps)]
fn handle() -> SyscallResult {
    Ok(0)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::handle;

    kernel_test!("roxy-syscall::getgid", getgid, {
        assert_eq!(handle(), Ok(0));
    });
}
