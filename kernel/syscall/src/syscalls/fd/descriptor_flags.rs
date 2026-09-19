use bitflags::bitflags;
use roxy_process::DescriptorError;

use crate::{args::SyscallArg, errno::Errno};

pub(super) const GET_SYSCALL: crate::Syscall = get::SYSCALL;
pub(super) const SET_SYSCALL: crate::Syscall = set::SYSCALL;

/// The descriptor flag word is private to this syscall; its first member therefore starts at bit
/// zero rather than using a foreign-numbering base.
const DESCRIPTOR_CLOSE_ON_EXEC: u64 = 1 << 0;

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct DescriptorFlags: u64 {
        const CLOSE_ON_EXEC = DESCRIPTOR_CLOSE_ON_EXEC;
    }
}

impl SyscallArg for DescriptorFlags {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        let unknown = raw & !Self::all().bits();
        if unknown != 0 {
            return Err(unsupported("descriptor-flags", unknown));
        }

        Ok(Self::from_bits_retain(raw))
    }
}

mod get {
    use roxy_fd::Fd;

    use super::{DescriptorFlags, map_process_error};
    use crate::{SyscallResult, numbers::SyscallNumber, syscall};

    syscall!(SyscallNumber::GetDescriptorFlags, handle(fd: Fd => BadFd));

    fn handle(fd: Fd) -> SyscallResult {
        let close_on_exec =
            roxy_process::descriptor_close_on_exec(fd).map_err(map_process_error)?;

        Ok(if close_on_exec {
            DescriptorFlags::CLOSE_ON_EXEC.bits()
        } else {
            0
        })
    }
}

mod set {
    use roxy_fd::Fd;

    use super::{DescriptorFlags, map_process_error};
    use crate::{SyscallResult, numbers::SyscallNumber, syscall};

    syscall!(SyscallNumber::SetDescriptorFlags, handle(
        fd: Fd => BadFd,
        flags: DescriptorFlags => Invalid,
    ));

    fn handle(fd: Fd, flags: DescriptorFlags) -> SyscallResult {
        roxy_process::set_descriptor_close_on_exec(
            fd,
            flags.contains(DescriptorFlags::CLOSE_ON_EXEC),
        )
        .map_err(map_process_error)?;

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

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{DESCRIPTOR_CLOSE_ON_EXEC, DescriptorFlags};
    use crate::{args::SyscallArg, errno::Errno};

    kernel_test!("roxy-syscall::descriptor-flags", parses_supported_flags, {
        assert_eq!(
            DescriptorFlags::parse(DESCRIPTOR_CLOSE_ON_EXEC, Errno::Invalid),
            Ok(DescriptorFlags::CLOSE_ON_EXEC)
        );
    });

    kernel_test!("roxy-syscall::descriptor-flags", rejects_unknown_flags, {
        assert_eq!(
            DescriptorFlags::parse(1, Errno::Invalid),
            Err(Errno::NotSupported)
        );
    });
}
