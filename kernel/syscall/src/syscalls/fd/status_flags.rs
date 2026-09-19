use roxy_process::DescriptorError;

use crate::errno::Errno;

pub(super) const GET_SYSCALL: crate::Syscall = get::SYSCALL;
pub(super) const SET_SYSCALL: crate::Syscall = set::SYSCALL;

mod get {
    use roxy_fd::Fd;

    use super::map_process_error;
    use crate::{SyscallResult, numbers::SyscallNumber, syscall};

    syscall!(SyscallNumber::GetStatusFlags, handle(fd: Fd => BadFd));

    fn handle(fd: Fd) -> SyscallResult {
        let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;

        Ok(file.status_flags().bits())
    }
}

mod set {
    use roxy_fd::{Fd, StatusFlags};

    use super::map_process_error;
    use crate::{SyscallResult, numbers::SyscallNumber, syscall};

    syscall!(SyscallNumber::SetStatusFlags, handle(
        fd: Fd => BadFd,
        flags: u64,
    ));

    /// Updates the file status flags of the open file description behind `fd`.
    ///
    /// Every bit outside `StatusFlags` is reported as unsupported; only the settable bits are
    /// changed, while access-mode bits remain fixed from open time.
    fn handle(fd: Fd, argument: u64) -> SyscallResult {
        let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;

        let unknown = argument & !StatusFlags::all().bits();
        let mut bit = 1;
        while bit != 0 {
            if unknown & bit != 0 {
                super::unsupported("status-flags", bit);
            }
            bit <<= 1;
        }

        let requested = StatusFlags::from_bits_retain(argument);
        let mut flags = file.status_flags();
        flags.remove(StatusFlags::SETTABLE);
        flags.insert(requested & StatusFlags::SETTABLE);

        file.set_status_flags(flags);

        Ok(0)
    }
}

fn map_process_error(error: DescriptorError) -> Errno {
    match error {
        DescriptorError::NotOpen => Errno::BadFd,
        DescriptorError::NoSpace => Errno::NoSpace,
    }
}

fn unsupported(operation: &str, argument: u64) -> Errno {
    crate::unsupported::unsupported_argument(operation, argument, Errno::NotSupported)
}
