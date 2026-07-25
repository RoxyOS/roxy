use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Geteuid, handle());

#[allow(clippy::unnecessary_wraps)]
fn handle() -> SyscallResult {
    Ok(0)
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::handle;

    kernel_test!("roxy-syscall::geteuid", geteuid, {
        assert_eq!(handle(), Ok(0));
    });
}
