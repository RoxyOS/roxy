mod action;
mod mask;
mod send;

use roxy_signal::Signal;

use crate::{Syscall, args::SyscallArg, errno::Errno, unsupported::unsupported_argument};

pub(super) const ACTION_SYSCALL: Syscall = action::SYSCALL;
pub(super) const MASK_SYSCALL: Syscall = mask::SYSCALL;
pub(super) const SEND_SYSCALL: Syscall = send::SYSCALL;

pub(crate) use mask::SignalMask;

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
            0 => return Err(unsupported_argument("signal", raw, Errno::NotSupported)),
            _ => return Err(error),
        };

        Ok(signal)
    }
}
