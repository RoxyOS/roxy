use core::mem::size_of;

use roxy_memory::UserAddress;
use roxy_process::{ProcessId, WaitError, WaitResult, WaitTarget};

use crate::{
    Syscall, SyscallResult, errno::Errno, numbers::SyscallNumber, unsupported::unsupported_argument,
};

pub(super) const SYSCALL: Syscall = Syscall::new(SyscallNumber::Waitpid, handle);

const WNOHANG: u64 = 1;

fn handle(arguments: [u64; 6]) -> SyscallResult {
    let target = parse_target(arguments[0])?;
    let status = parse_status(arguments[1])?;
    let no_hang = parse_no_hang(arguments[2])?;

    if arguments[3] != 0 {
        return Err(unsupported_argument(
            "waitpid.rusage",
            arguments[3],
            Errno::NotSupported,
        ));
    }

    let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;
    if let Some(status) = status {
        addrspace
            .validate_writable(status, size_of::<u32>())
            .map_err(|_| Errno::Fault)?;
    }

    let (process_id, exit_status) =
        match roxy_process::wait_current(target, no_hang).map_err(map_wait_error)? {
            WaitResult::Exited { process_id, status } => (process_id, status),
            WaitResult::Pending => return Ok(0),
        };

    if let Some(status) = status {
        addrspace
            .write_bytes(status, &encode_status(exit_status.code()).to_ne_bytes())
            .map_err(|_| Errno::Fault)?;
    }

    Ok(process_id.as_u64())
}

fn parse_target(raw: u64) -> Result<WaitTarget, Errno> {
    let pid = i32::try_from(raw.cast_signed()).map_err(|_| Errno::Invalid)?;

    match pid {
        -1 => Ok(WaitTarget::Any),
        1.. => Ok(WaitTarget::Process(
            ProcessId::new(pid.cast_unsigned().into()).unwrap(),
        )),
        _ => Err(unsupported_argument(
            "waitpid.pid-selector",
            pid,
            Errno::NotSupported,
        )),
    }
}

fn parse_status(raw: u64) -> Result<Option<UserAddress>, Errno> {
    match raw {
        0 => Ok(None),
        raw => UserAddress::new(raw).map(Some).ok_or(Errno::Fault),
    }
}

fn parse_no_hang(raw: u64) -> Result<bool, Errno> {
    match raw {
        0 => Ok(false),
        WNOHANG => Ok(true),
        _ => Err(unsupported_argument(
            "waitpid.options",
            raw,
            Errno::NotSupported,
        )),
    }
}

fn encode_status(code: u8) -> u32 {
    u32::from(code) << 8
}

const fn map_wait_error(error: WaitError) -> Errno {
    match error {
        WaitError::NoChild => Errno::Child,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::encode_status;

    kernel_test!("roxy-syscall::waitpid-status", waitpid_status, {
        assert_eq!(encode_status(0), 0);
        assert_eq!(encode_status(23), 0x1700);
        assert_eq!(encode_status(u8::MAX), 0xff00);
    });
}
