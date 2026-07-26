use roxy_signal::Signal;

use super::SyscallArg;
use crate::{errno::Errno, unsupported::unsupported_argument};

impl SyscallArg for Signal {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let signal = match raw {
            1 => Signal::Hangup,
            2 => Signal::Interrupt,
            3 => Signal::Quit,
            4 => Signal::IllegalInstruction,
            6 => Signal::Abort,
            7 => Signal::BusError,
            8 => Signal::FloatingPointException,
            9 => Signal::Kill,
            10 => Signal::User1,
            11 => Signal::SegmentationFault,
            12 => Signal::User2,
            13 => Signal::BrokenPipe,
            14 => Signal::Alarm,
            15 => Signal::Terminate,
            17 => Signal::Child,
            18 => Signal::Continue,
            19 => Signal::Stop,
            20 => Signal::TerminalStop,
            21 => Signal::TerminalInput,
            22 => Signal::TerminalOutput,
            28 => Signal::WindowChanged,
            0 => {
                return Err(unsupported_argument("signal", raw, Errno::NotSupported));
            }
            _ => return Err(error),
        };

        Ok(signal)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_signal::Signal;
    use roxy_test::kernel_test;

    use super::SyscallArg;
    use crate::errno::Errno;

    kernel_test!("roxy-syscall::signal-argument", validates_signal, {
        assert_eq!(Signal::parse(15, Errno::Invalid), Ok(Signal::Terminate));
        assert_eq!(Signal::parse(0, Errno::Invalid), Err(Errno::NotSupported));
        assert_eq!(Signal::parse(5, Errno::Invalid), Err(Errno::Invalid));
    });
}
