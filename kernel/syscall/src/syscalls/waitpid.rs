use roxy_process::{ProcessId, WaitError, WaitResult, WaitTarget};

use crate::{
    SyscallResult,
    args::{Out, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
    unsupported::unsupported_argument,
};

syscall!(SyscallNumber::Waitpid, handle(target: WaitTarget => Invalid, status: Option<Out<u32>> => Fault, options: WaitOptions => Invalid, rusage: u64));

const WNOHANG: u64 = 1;

#[derive(Clone, Copy)]
enum WaitOptions {
    Blocking,
    NoHang,
}

fn handle(
    target: WaitTarget,
    status: Option<Out<u32>>,
    options: WaitOptions,
    rusage: u64,
) -> SyscallResult {
    let no_hang = matches!(options, WaitOptions::NoHang);

    if rusage != 0 {
        return Err(unsupported_argument(
            "waitpid.rusage",
            rusage,
            Errno::NotSupported,
        ));
    }

    if let Some(status) = status {
        status.validate()?;
    }

    let (process_id, exit_status) =
        match roxy_process::wait_current(target, no_hang).map_err(map_wait_error)? {
            WaitResult::Exited { process_id, status } => (process_id, status),
            WaitResult::Pending => return Ok(0),
        };

    if let Some(output) = status {
        let encoded = encode_status(exit_status.code());

        // SAFETY: u32 has no padding and encoded is initialized.
        unsafe { output.write(&encoded) }?;
    }

    Ok(process_id.as_u64())
}

impl SyscallArg for WaitTarget {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let pid = i32::try_from(raw.cast_signed()).map_err(|_| error)?;

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
}

impl SyscallArg for WaitOptions {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            0 => Ok(Self::Blocking),
            WNOHANG => Ok(Self::NoHang),
            _ => Err(unsupported_argument(
                "waitpid.options",
                raw,
                Errno::NotSupported,
            )),
        }
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
