use roxy_fd::{Fd, TruncateError};

use crate::{SyscallResult, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Ftruncate, handle(fd: Fd => BadFd, size: u64));

fn handle(fd: Fd, size: u64) -> SyscallResult {
    if size > i64::MAX.cast_unsigned() {
        return Err(Errno::Invalid);
    }

    let file = roxy_process::current_open_file(fd).map_err(|_| Errno::BadFd)?;
    file.truncate(size).map_err(map_truncate_error)?;

    Ok(0)
}

fn map_truncate_error(error: TruncateError) -> Errno {
    match error {
        TruncateError::PermissionDenied => Errno::BadFd,
        TruncateError::BadOperation | TruncateError::InvalidSize => Errno::Invalid,
        TruncateError::ReadOnly => Errno::ReadOnly,
        TruncateError::NoSpace => Errno::NoSpace,
        TruncateError::Io => Errno::Io,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::TruncateError;
    use roxy_test::kernel_test;

    use super::map_truncate_error;
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::ftruncate-errors", maps_errors, {
        assert_eq!(
            map_truncate_error(TruncateError::PermissionDenied),
            Errno::BadFd
        );
        assert_eq!(
            map_truncate_error(TruncateError::BadOperation),
            Errno::Invalid
        );
        assert_eq!(map_truncate_error(TruncateError::ReadOnly), Errno::ReadOnly);
        assert_eq!(map_truncate_error(TruncateError::NoSpace), Errno::NoSpace);
    });
}
