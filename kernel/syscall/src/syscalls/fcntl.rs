use roxy_fd::{Fd, StatusFlags};
use roxy_process::{self, DescriptorError};

use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Fcntl, handle(
    fd: Fd => BadFd,
    command: FcntlCommand => NotSupported,
    argument: u64,
));

/// The `fcntl` commands this kernel supports, matching the values in `abi-bits/fcntl.h`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FcntlCommand {
    DupFd = 0,
    GetFd = 1,
    SetFd = 2,
    GetFl = 3,
    SetFl = 4,
    DupFdCloexec = 1030,
}

impl FcntlCommand {
    fn parse(raw: u64) -> Result<Self, Errno> {
        match raw {
            0 => Ok(Self::DupFd),
            1 => Ok(Self::GetFd),
            2 => Ok(Self::SetFd),
            3 => Ok(Self::GetFl),
            4 => Ok(Self::SetFl),
            1030 => Ok(Self::DupFdCloexec),
            _ => Err(unsupported("fcntl.command", raw)),
        }
    }
}

impl SyscallArg for FcntlCommand {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        Self::parse(raw)
    }
}

fn handle(fd: Fd, command: FcntlCommand, argument: u64) -> SyscallResult {
    match command {
        FcntlCommand::GetFd => handle_getfd(fd),
        FcntlCommand::SetFd => handle_setfd(fd, argument),
        FcntlCommand::GetFl => handle_getfl(fd),
        FcntlCommand::SetFl => handle_setfl(fd, argument),
        FcntlCommand::DupFd => handle_dupfd(fd, argument, false),
        FcntlCommand::DupFdCloexec => handle_dupfd(fd, argument, true),
    }
}

/// Returns the descriptor flags (`FD_CLOEXEC` is 1) of `fd`.
fn handle_getfd(fd: Fd) -> SyscallResult {
    let close_on_exec = roxy_process::fcntl_close_on_exec(fd).map_err(map_process_error)?;

    Ok(u64::from(close_on_exec))
}

/// Sets the descriptor flags of `fd`; only `FD_CLOEXEC` (1) is recognized.
fn handle_setfd(fd: Fd, argument: u64) -> SyscallResult {
    let close_on_exec = argument & 1 != 0;

    roxy_process::fcntl_set_close_on_exec(fd, close_on_exec).map_err(map_process_error)?;

    Ok(0)
}

/// Returns the file status flags of the open file description behind `fd`.
fn handle_getfl(fd: Fd) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;

    Ok(file.status_flags().bits())
}

/// Updates the file status flags of the open file description behind `fd`.
///
/// Only the bits in `StatusFlags::SETTABLE` are changed (currently append and large-file
/// mode); access mode bits and other status flags are preserved. `O_NONBLOCK` is not yet
/// modeled, so requesting it is silently ignored.
fn handle_setfl(fd: Fd, argument: u64) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;

    let requested = StatusFlags::from_bits_retain(argument);
    let mut flags = file.status_flags();
    flags.remove(StatusFlags::SETTABLE);
    flags.insert(requested & StatusFlags::SETTABLE);

    file.set_status_flags(flags);

    Ok(0)
}

/// Duplicates `fd` to the lowest available descriptor at or above `argument`.
fn handle_dupfd(fd: Fd, argument: u64, close_on_exec: bool) -> SyscallResult {
    let minimum = Fd::new(u32::try_from(argument).map_err(|_| Errno::Invalid)?);
    let newfd = roxy_process::fcntl_dupfd(fd, minimum, close_on_exec).map_err(map_process_error)?;

    Ok(u64::from(newfd.as_u32()))
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

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::FcntlCommand;
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::fcntl-command", parses_supported_commands, {
        assert_eq!(FcntlCommand::parse(0), Ok(FcntlCommand::DupFd));
        assert_eq!(FcntlCommand::parse(1), Ok(FcntlCommand::GetFd));
        assert_eq!(FcntlCommand::parse(2), Ok(FcntlCommand::SetFd));
        assert_eq!(FcntlCommand::parse(3), Ok(FcntlCommand::GetFl));
        assert_eq!(FcntlCommand::parse(4), Ok(FcntlCommand::SetFl));
        assert_eq!(FcntlCommand::parse(1030), Ok(FcntlCommand::DupFdCloexec));
    });

    kernel_test!(
        "roxy-syscall::fcntl-command",
        rejects_unsupported_commands,
        {
            assert_eq!(FcntlCommand::parse(1000), Err(Errno::NotSupported));
        }
    );
}
