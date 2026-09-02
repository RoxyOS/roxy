use roxy_process::{ProcessId, WaitError, WaitResult, WaitTarget};

use crate::{
    SyscallResult,
    args::{Nullable, Out, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
    unsupported::unsupported_argument,
};

syscall!(SyscallNumber::Waitpid, handle(target: WaitTarget => Invalid, status: Nullable<Out<u32>> => Fault, options: WaitOptions => Invalid, rusage: u64));

const WNOHANG: u64 = 1;
const WUNTRACED: u64 = 2;
const WCONTINUED: u64 = 8;

/// Linux `waitpid` option bits, validated against the `WNOHANG`/`WUNTRACED`/`WCONTINUED`
/// constants above.
#[derive(Clone, Copy)]
struct WaitOptions {
    no_hang: bool,
    wuntraced: bool,
    wcontinued: bool,
}

fn handle(
    target: WaitTarget,
    status: Nullable<Out<u32>>,
    options: WaitOptions,
    rusage: u64,
) -> SyscallResult {
    let status = status.into_option();

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

    let wait_options = roxy_process::WaitOptions {
        no_hang: options.no_hang,
        wuntraced: options.wuntraced,
        wcontinued: options.wcontinued,
    };

    match roxy_process::wait_current(target, wait_options).map_err(map_wait_error)? {
        WaitResult::Exited {
            process_id,
            status: exit_status,
        } => {
            if let Some(output) = status {
                let encoded = encode_status(exit_status);

                // SAFETY: u32 has no padding and encoded is initialized.
                unsafe { output.write(&encoded) }?;
            }

            Ok(process_id.as_u64())
        }
        WaitResult::Stopped { process_id, signal } => {
            if let Some(output) = status {
                let encoded = encode_stopped_status(signal);

                // SAFETY: u32 has no padding and encoded is initialized.
                unsafe { output.write(&encoded) }?;
            }

            Ok(process_id.as_u64())
        }
        WaitResult::Continued { process_id } => {
            if let Some(output) = status {
                let encoded = encode_continued_status();

                // SAFETY: u32 has no padding and encoded is initialized.
                unsafe { output.write(&encoded) }?;
            }

            Ok(process_id.as_u64())
        }
        WaitResult::Pending => Ok(0),
    }
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
        let unknown = raw & !(WNOHANG | WUNTRACED | WCONTINUED);
        if unknown != 0 {
            return Err(unsupported_argument(
                "waitpid.options",
                unknown,
                Errno::NotSupported,
            ));
        }

        Ok(Self {
            no_hang: raw & WNOHANG != 0,
            wuntraced: raw & WUNTRACED != 0,
            wcontinued: raw & WCONTINUED != 0,
        })
    }
}

fn encode_status(status: roxy_process::ExitStatus) -> u32 {
    match status {
        roxy_process::ExitStatus::Exited(code) => u32::from(code) << 8,
        roxy_process::ExitStatus::Signaled(signal) => u32::from(signal.number()),
    }
}

/// Encodes a stopped child's status: low byte `0x7f` with the stopping signal in bits 8-15
/// (`WIFSTOPPED` + `WSTOPSIG`).
fn encode_stopped_status(signal: roxy_signal::Signal) -> u32 {
    0x7f | (u32::from(signal.number()) << 8)
}

/// Encodes a continued child's status: the all-ones `0xffff` pattern that `WIFCONTINUED`
/// recognizes.
const fn encode_continued_status() -> u32 {
    0xffff
}

const fn map_wait_error(error: WaitError) -> Errno {
    match error {
        WaitError::NoChild => Errno::Child,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use roxy_process::ExitStatus;
    use roxy_signal::Signal;

    use super::{encode_status, encode_stopped_status};

    kernel_test!("roxy-syscall::waitpid-status", waitpid_status, {
        assert_eq!(encode_status(ExitStatus::exited(0)), 0);
        assert_eq!(encode_status(ExitStatus::exited(23)), 0x1700);
        assert_eq!(
            encode_status(ExitStatus::exited(u64::from(u8::MAX))),
            0xff00
        );
        assert_eq!(encode_status(ExitStatus::signaled(Signal::Terminate)), 15);
    });

    kernel_test!("roxy-syscall::waitpid-stopped-status", waitpid_stopped, {
        assert_eq!(
            encode_stopped_status(Signal::TerminalStop),
            0x7f | (20 << 8)
        );
        assert_eq!(encode_stopped_status(Signal::Stop), 0x7f | (19 << 8));
    });
}
