use roxy_fd::{Fd, StatusFlags};
use roxy_process::{self, DescriptorError};

use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Fcntl, handle(
    fd: Fd => BadFd,
    command: FcntlCommand => NotSupported,
    argument: u64,
));

/// Base of the Roxy `fcntl` command space; a supported command's value is this base plus the
/// command's index.
///
/// The base sits above every command number another personality uses, so a command below it can
/// only be another personality's numbering and the handler can report that instead of misreading
/// it as an unrelated command of its own. Linux's highest `fcntl` command is `F_GET_SEALS` at
/// 1034.
const COMMAND_BASE: u64 = 0x1000;

/// The value `abi-bits/fcntl.h` defines every command this kernel does not implement to.
///
/// Pinning them all to one value keeps ported sources compiling while making the call unmissable
/// at runtime, and keeps them out of the supported range.
const UNSUPPORTED_COMMAND: u64 = 0x100;

/// Every unsupported command lies below the base, so no value in the supported range is one of
/// them.
const _: () = assert!(UNSUPPORTED_COMMAND < COMMAND_BASE);

/// The `fcntl` commands this kernel supports, numbered from zero; the value on the wire is
/// [`COMMAND_BASE`] plus the index. The indices match `abi-bits/fcntl.h`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FcntlCommand {
    DupFd = 0,
    GetFd = 1,
    SetFd = 2,
    GetFl = 3,
    SetFl = 4,
    DupFdCloexec = 5,
}

impl FcntlCommand {
    fn parse(raw: u64) -> Result<Self, Errno> {
        let Some(index) = raw.checked_sub(COMMAND_BASE) else {
            return Err(unsupported_command(raw));
        };

        match index {
            0 => Ok(Self::DupFd),
            1 => Ok(Self::GetFd),
            2 => Ok(Self::SetFd),
            3 => Ok(Self::GetFl),
            4 => Ok(Self::SetFl),
            5 => Ok(Self::DupFdCloexec),
            _ => Err(unsupported("fcntl.command", raw)),
        }
    }
}

/// Reports a command below the base as the two cases that can produce one.
///
/// Below the base is either the header's [`UNSUPPORTED_COMMAND`] marker for a command this kernel
/// deliberately does not implement, or a foreign personality's numbering from a program compiled
/// against another libc's header. Naming them separately keeps a stale caller visible in the
/// diagnostic stream instead of looking like an ordinary unknown command.
fn unsupported_command(raw: u64) -> Errno {
    if raw == UNSUPPORTED_COMMAND {
        unsupported("fcntl.command.unsupported", raw)
    } else {
        unsupported("fcntl.command.foreign", raw)
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

/// Common `O_*` file status flags that are not yet modeled by the kernel. Each is reported
/// through `unsupported()` when requested via `F_SETFL`, rather than being dropped silently.
const UNIMPLEMENTED_STATUS_FLAGS: [u64; 4] = [
    0o20000,      // O_ASYNC
    0o40000,      // O_DIRECT
    0o1_000_000,  // O_NOATIME
    0o10_000_000, // O_SYNC
];

/// Updates the file status flags of the open file description behind `fd`.
///
/// Only the bits in `StatusFlags::SETTABLE` are changed (currently append and large-file
/// mode); access mode bits are preserved as legitimate, non-modifiable flags. Any other bit
/// (e.g. `O_NONBLOCK`) is unimplemented and is reported through `unsupported()` rather than
/// dropped silently.
fn handle_setfl(fd: Fd, argument: u64) -> SyscallResult {
    let file = roxy_process::current_open_file(fd).map_err(map_process_error)?;

    let requested = StatusFlags::from_bits_retain(argument);
    let mut flags = file.status_flags();
    flags.remove(StatusFlags::SETTABLE);
    flags.insert(requested & StatusFlags::SETTABLE);

    file.set_status_flags(flags);

    // Report every unimplemented flag bit rather than silently ignoring it. Access mode bits
    // (WRITE_ONLY/READ_WRITE) are excluded: F_SETFL legally preserves them.
    let access_mode = StatusFlags::WRITE_ONLY.bits() | StatusFlags::READ_WRITE.bits();
    let unsupported_bits = argument & !StatusFlags::SETTABLE.bits() & !access_mode;
    for bit in UNIMPLEMENTED_STATUS_FLAGS {
        if unsupported_bits & bit != 0 {
            unsupported("fcntl.setfl", bit);
        }
    }

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

    use super::{COMMAND_BASE, FcntlCommand, UNSUPPORTED_COMMAND};
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::fcntl-command", parses_supported_commands, {
        assert_eq!(FcntlCommand::parse(COMMAND_BASE), Ok(FcntlCommand::DupFd));
        assert_eq!(
            FcntlCommand::parse(COMMAND_BASE + 1),
            Ok(FcntlCommand::GetFd)
        );
        assert_eq!(
            FcntlCommand::parse(COMMAND_BASE + 2),
            Ok(FcntlCommand::SetFd)
        );
        assert_eq!(
            FcntlCommand::parse(COMMAND_BASE + 3),
            Ok(FcntlCommand::GetFl)
        );
        assert_eq!(
            FcntlCommand::parse(COMMAND_BASE + 4),
            Ok(FcntlCommand::SetFl)
        );
        assert_eq!(
            FcntlCommand::parse(COMMAND_BASE + 5),
            Ok(FcntlCommand::DupFdCloexec)
        );
    });

    kernel_test!(
        "roxy-syscall::fcntl-command",
        rejects_commands_outside_the_space,
        {
            // The header's marker for a command the kernel does not implement.
            assert_eq!(
                FcntlCommand::parse(UNSUPPORTED_COMMAND),
                Err(Errno::NotSupported)
            );

            // Another personality's numbering: Linux `F_DUPFD` is 0, `F_GETFD` is 1, and
            // `F_DUPFD_CLOEXEC` is 1030, all below the base.
            for foreign in [0, 1, 2, 3, 4, 1030, COMMAND_BASE - 1] {
                assert_eq!(FcntlCommand::parse(foreign), Err(Errno::NotSupported));
            }

            // A well-formed Roxy command the kernel does not define.
            assert_eq!(
                FcntlCommand::parse(COMMAND_BASE + 6),
                Err(Errno::NotSupported)
            );
        }
    );
}
