use roxy_fd::Fd;
use roxy_process::DescriptorError;

use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Listen, handle(fd: Fd => BadFd, backlog: Backlog => Invalid));

/// Maximum pending-connection capacity, mirroring Linux's `somaxconn` clamp.
const MAX_BACKLOG: u32 = 4096;

/// A connection backlog clamped to the supported range.
///
/// Linux clamps negative backlogs to zero and caps the value at its `somaxconn` limit instead of
/// rejecting them; parsing applies the same normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Backlog(u32);

impl SyscallArg for Backlog {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        let backlog = raw.cast_signed().clamp(0, i64::from(MAX_BACKLOG));
        let backlog = u32::try_from(backlog).map_err(|_| Errno::Invalid)?;

        Ok(Self(backlog))
    }
}

fn handle(fd: Fd, backlog: Backlog) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_descriptor_error)?;
    let Backlog(backlog) = backlog;

    file.socket_ops(|socket| socket.listen(backlog))
        .ok_or(Errno::NotSocket)?
        .map_err(super::map_socket_error)?;

    Ok(0)
}

fn map_descriptor_error(_: DescriptorError) -> Errno {
    Errno::BadFd
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{Backlog, MAX_BACKLOG};
    use crate::args::SyscallArg;
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::listen-backlog", clamps_to_supported_range, {
        assert_eq!(Backlog::parse(0, Errno::Invalid), Ok(Backlog(0)));
        assert_eq!(Backlog::parse(128, Errno::Invalid), Ok(Backlog(128)));
        assert_eq!(
            Backlog::parse(1 << 40, Errno::Invalid),
            Ok(Backlog(MAX_BACKLOG))
        );
        assert_eq!(Backlog::parse(u64::MAX, Errno::Invalid), Ok(Backlog(0)));
    });
}
