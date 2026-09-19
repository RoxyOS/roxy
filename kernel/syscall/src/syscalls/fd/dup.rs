use bitflags::bitflags;
use roxy_fd::Fd;
use roxy_process::{self, DescriptorError};

use crate::{SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall};

syscall!(SyscallNumber::Dup, handle(
    oldfd: Fd => BadFd,
    options: DupOptions => Invalid,
    minimum_fd: u64,
));

/// The Roxy `dup` option word is private to this syscall, so its options start at bit zero.
const DUP_CLOSE_ON_EXEC: u64 = 1 << 0;
const DUP_MINIMUM_ARGUMENT: u64 = 1 << 1;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct DupOptions: u64 {
        const CLOSE_ON_EXEC = DUP_CLOSE_ON_EXEC;
        /// Set this bit to use the `minimum_fd` argument as the lowest descriptor searched for
        /// the duplicate; when clear, duplication searches from descriptor zero.
        const MINIMUM_ARGUMENT = DUP_MINIMUM_ARGUMENT;
    }
}

impl SyscallArg for DupOptions {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        let unknown = raw & !Self::all().bits();
        if unknown != 0 {
            return Err(unsupported("dup.options", unknown));
        }

        Ok(Self::from_bits_retain(raw))
    }
}

fn handle(oldfd: Fd, options: DupOptions, minimum_fd: u64) -> SyscallResult {
    let minimum = if options.contains(DupOptions::MINIMUM_ARGUMENT) {
        Fd::new(u32::try_from(minimum_fd).map_err(|_| Errno::Invalid)?)
    } else {
        Fd::new(0)
    };

    let newfd = roxy_process::duplicate_at_or_above(
        oldfd,
        minimum,
        options.contains(DupOptions::CLOSE_ON_EXEC),
    )
    .map_err(map_process_error)?;

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

    use super::{DUP_CLOSE_ON_EXEC, DUP_MINIMUM_ARGUMENT, DupOptions};
    use crate::{args::SyscallArg, errno::Errno};

    kernel_test!("roxy-syscall::dup-options", parses_supported_options, {
        assert_eq!(
            DupOptions::parse(DUP_CLOSE_ON_EXEC, Errno::Invalid),
            Ok(DupOptions::CLOSE_ON_EXEC)
        );
        assert_eq!(
            DupOptions::parse(DUP_MINIMUM_ARGUMENT, Errno::Invalid),
            Ok(DupOptions::MINIMUM_ARGUMENT)
        );
        assert_eq!(
            DupOptions::parse(DUP_CLOSE_ON_EXEC | DUP_MINIMUM_ARGUMENT, Errno::Invalid),
            Ok(DupOptions::CLOSE_ON_EXEC | DupOptions::MINIMUM_ARGUMENT)
        );
    });

    kernel_test!("roxy-syscall::dup-options", rejects_unknown_options, {
        assert_eq!(
            DupOptions::parse(1, Errno::Invalid),
            Err(Errno::NotSupported)
        );
    });
}
