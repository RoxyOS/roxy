use core::mem::{align_of, offset_of, size_of};

use roxy_process::{ExitStatus, ProcessId, WaitError, WaitResult, WaitTarget};
use roxy_signal::Signal;

use crate::{
    SyscallResult,
    args::{Nullable, Out, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
    unsupported::unsupported_argument,
};

syscall!(SyscallNumber::Waitpid, handle(target: WaitTarget => Invalid, status: Nullable<Out<WaitStatusAbi>> => Fault, options: WaitOptions => Invalid, rusage: u64));

/// Kind word of [`WaitStatusAbi`]: which state change the record reports.
///
/// The kernel produces this word and only the libc reads it, so it needs no base above another
/// personality's numbering the way a userspace-supplied word does. Zero is reserved instead, so an
/// all-zero record is never a valid status.
const WAIT_KIND_EXITED: u32 = 1;
const WAIT_KIND_SIGNALED: u32 = 2;
const WAIT_KIND_STOPPED: u32 = 3;
const WAIT_KIND_CONTINUED: u32 = 4;

/// Fixed-layout `waitpid` status record copied across the userspace syscall ABI.
///
/// A flat record rather than the POSIX wait-status word `WIFEXITED` and its neighbours decode:
/// the kernel already holds the state change as typed values, and one member per value keeps every
/// offset constant instead of an overlay the kind selects. Encoding the word is the libc's job, so
/// its bit layout stops at this boundary. The record is mirrored by
/// `sysdeps/roxy/include/roxy/syscall.h` in the Roxy mlibc fork.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WaitStatusAbi {
    kind: u32,
    /// The exit code for [`WAIT_KIND_EXITED`], the signal number for [`WAIT_KIND_SIGNALED`] and
    /// [`WAIT_KIND_STOPPED`], and zero otherwise.
    code: u32,
}

const _: () = assert!(size_of::<WaitStatusAbi>() == 8);
const _: () = assert!(align_of::<WaitStatusAbi>() == 4);
const _: () = assert!(offset_of!(WaitStatusAbi, kind) == 0);
const _: () = assert!(offset_of!(WaitStatusAbi, code) == 4);

impl WaitStatusAbi {
    /// Reports a child that returned an exit code.
    const fn exited(code: u8) -> Self {
        Self {
            kind: WAIT_KIND_EXITED,
            code: code as u32,
        }
    }

    /// Reports a child terminated by a signal.
    const fn signaled(signal: Signal) -> Self {
        Self {
            kind: WAIT_KIND_SIGNALED,
            code: signal.number() as u32,
        }
    }

    /// Reports the signal that stopped a child.
    const fn stopped(signal: Signal) -> Self {
        Self {
            kind: WAIT_KIND_STOPPED,
            code: signal.number() as u32,
        }
    }

    /// Reports a child resumed by `SIGCONT`, which carries no signal of its own.
    const fn continued() -> Self {
        Self {
            kind: WAIT_KIND_CONTINUED,
            code: 0,
        }
    }
}

/// Roxy numbers `waitpid` option bits from a base above Linux's *whole* option range, whose
/// highest is `WNOWAIT` at bit 24, so no Linux bit can alias one of ours.
const WAIT_OPTIONS_BASE: u64 = 1 << 25;

const WNOHANG: u64 = WAIT_OPTIONS_BASE;
const WUNTRACED: u64 = WAIT_OPTIONS_BASE << 1;
const WCONTINUED: u64 = WAIT_OPTIONS_BASE << 2;

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
    status: Nullable<Out<WaitStatusAbi>>,
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

    let result = roxy_process::wait_current(target, wait_options).map_err(map_wait_error)?;
    let Some((process_id, record)) = status_record(result) else {
        // A non-blocking wait that observed no state change writes nothing; the caller sees the
        // zero return and must not read the output slot.
        return Ok(0);
    };

    if let Some(output) = status {
        // SAFETY: WaitStatusAbi's checked repr(C) layout consists of initialized integer fields
        // without implicit padding.
        unsafe { output.write(&record) }?;
    }

    Ok(process_id.as_u64())
}

/// The state change one wait result reports, or `None` when no child changed state.
///
/// The kernel reports the change itself; `Pending` carries none, and run-to-completion of the
/// operation leaves no other case.
fn status_record(result: WaitResult) -> Option<(ProcessId, WaitStatusAbi)> {
    match result {
        WaitResult::Exited { process_id, status } => {
            let record = match status {
                ExitStatus::Exited(code) => WaitStatusAbi::exited(code),
                ExitStatus::Signaled(signal) => WaitStatusAbi::signaled(signal),
            };

            Some((process_id, record))
        }
        WaitResult::Stopped { process_id, signal } => {
            Some((process_id, WaitStatusAbi::stopped(signal)))
        }
        WaitResult::Continued { process_id } => Some((process_id, WaitStatusAbi::continued())),
        WaitResult::Pending => None,
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
        // Anything below the Roxy base is another personality's numbering: Linux numbers its
        // option bits from 0, so a Linux-valued word lands entirely in the foreign zone.
        let foreign = raw & (WAIT_OPTIONS_BASE - 1);
        if foreign != 0 {
            return Err(unsupported_argument(
                "waitpid.options.foreign",
                foreign,
                Errno::NotSupported,
            ));
        }

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

const fn map_wait_error(error: WaitError) -> Errno {
    match error {
        WaitError::NoChild => Errno::Child,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_process::{ExitStatus, ProcessId, WaitResult};
    use roxy_signal::Signal;
    use roxy_test::kernel_test;

    use super::{
        WAIT_KIND_CONTINUED, WAIT_KIND_EXITED, WAIT_KIND_SIGNALED, WAIT_KIND_STOPPED,
        WaitStatusAbi, status_record,
    };

    kernel_test!("roxy-syscall::waitpid-status", waitpid_status, {
        let process_id = ProcessId::new(7).unwrap();

        assert_eq!(
            status_record(WaitResult::Exited {
                process_id,
                status: ExitStatus::exited(0),
            }),
            Some((
                process_id,
                WaitStatusAbi {
                    kind: WAIT_KIND_EXITED,
                    code: 0
                }
            ))
        );
        assert_eq!(
            status_record(WaitResult::Exited {
                process_id,
                status: ExitStatus::exited(23),
            }),
            Some((
                process_id,
                WaitStatusAbi {
                    kind: WAIT_KIND_EXITED,
                    code: 23
                }
            ))
        );
        assert_eq!(
            status_record(WaitResult::Exited {
                process_id,
                status: ExitStatus::exited(u64::from(u8::MAX)),
            }),
            Some((
                process_id,
                WaitStatusAbi {
                    kind: WAIT_KIND_EXITED,
                    code: 255
                }
            ))
        );
        assert_eq!(
            status_record(WaitResult::Exited {
                process_id,
                status: ExitStatus::signaled(Signal::Terminate),
            }),
            Some((
                process_id,
                WaitStatusAbi {
                    kind: WAIT_KIND_SIGNALED,
                    code: 15
                }
            ))
        );
        assert_eq!(
            status_record(WaitResult::Stopped {
                process_id,
                signal: Signal::TerminalStop,
            }),
            Some((
                process_id,
                WaitStatusAbi {
                    kind: WAIT_KIND_STOPPED,
                    code: 20
                }
            ))
        );
        assert_eq!(
            status_record(WaitResult::Stopped {
                process_id,
                signal: Signal::Stop,
            }),
            Some((
                process_id,
                WaitStatusAbi {
                    kind: WAIT_KIND_STOPPED,
                    code: 19
                }
            ))
        );
        assert_eq!(
            status_record(WaitResult::Continued { process_id }),
            Some((
                process_id,
                WaitStatusAbi {
                    kind: WAIT_KIND_CONTINUED,
                    code: 0
                }
            ))
        );

        // No state change means no record: the output slot is left alone.
        assert_eq!(status_record(WaitResult::Pending), None);
    });
}
