use roxy_vfs::FilePermissions;

use crate::{SyscallResult, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Umask, handle(permissions: FilePermissions => Invalid));

#[allow(clippy::unnecessary_wraps)]
fn handle(permissions: FilePermissions) -> SyscallResult {
    // `umask(2)` returns the previous value, so surface what was replaced, not the new mask.
    let old = roxy_process::replace_current_umask(permissions);

    Ok(u64::from(old.bits()))
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;
    use roxy_vfs::FilePermissions;

    use super::handle;

    kernel_test!("roxy-syscall::umask", returns_previous_umask, {
        let previous = roxy_process::current_umask();

        assert_eq!(
            handle(FilePermissions::new(0o077).unwrap()),
            Ok(u64::from(previous.bits()))
        );
    });
}
