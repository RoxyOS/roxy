mod action;
mod mask;
mod send;

use alloc::vec::Vec;
use core::mem;

use bitflags::bitflags;
use roxy_memory::UserAddress;
use roxy_signal::Signal;
use strum::IntoEnumIterator;

use crate::{
    Syscall,
    args::{SyscallArg, user_memory},
    errno::Errno,
    unsupported::unsupported_argument,
};

pub(super) const ACTION_SYSCALL: Syscall = action::SYSCALL;
pub(super) const MASK_SYSCALL: Syscall = mask::SYSCALL;
pub(super) const SEND_SYSCALL: Syscall = send::SYSCALL;

#[repr(C)]
#[derive(Clone, Copy)]
struct SignalSetAbi {
    bits: [u64; 16],
}

const _: () = assert!(mem::size_of::<SignalSetAbi>() == 128);

const fn signal_bit(signal: Signal) -> u64 {
    1 << (signal.number() - 1)
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct SignalSet: u64 {
        const HANGUP = signal_bit(Signal::Hangup);
        const INTERRUPT = signal_bit(Signal::Interrupt);
        const QUIT = signal_bit(Signal::Quit);
        const ILLEGAL_INSTRUCTION = signal_bit(Signal::IllegalInstruction);
        const ABORT = signal_bit(Signal::Abort);
        const BUS_ERROR = signal_bit(Signal::BusError);
        const FLOATING_POINT_EXCEPTION = signal_bit(Signal::FloatingPointException);
        const KILL = signal_bit(Signal::Kill);
        const USER1 = signal_bit(Signal::User1);
        const SEGMENTATION_FAULT = signal_bit(Signal::SegmentationFault);
        const USER2 = signal_bit(Signal::User2);
        const BROKEN_PIPE = signal_bit(Signal::BrokenPipe);
        const ALARM = signal_bit(Signal::Alarm);
        const TERMINATE = signal_bit(Signal::Terminate);
        const CHILD = signal_bit(Signal::Child);
        const CONTINUE = signal_bit(Signal::Continue);
        const STOP = signal_bit(Signal::Stop);
        const TERMINAL_STOP = signal_bit(Signal::TerminalStop);
        const TERMINAL_INPUT = signal_bit(Signal::TerminalInput);
        const TERMINAL_OUTPUT = signal_bit(Signal::TerminalOutput);
        const WINDOW_CHANGED = signal_bit(Signal::WindowChanged);
    }
}

impl SignalSet {
    pub(crate) fn to_vec(self) -> Vec<Signal> {
        Signal::iter()
            .filter(|signal| self.bits() & signal_bit(*signal) != 0)
            .collect()
    }
}

impl SyscallArg for SignalSet {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut set = SignalSetAbi { bits: [0; 16] };

        // SAFETY: SignalSetAbi has a checked C layout and is fully initialized.
        unsafe { user_memory::read(address, &mut set) }?;

        if set.bits[1..].iter().any(|bits| *bits != 0) {
            return Err(unsupported_argument(
                "signal_set.extended_bits",
                "set",
                Errno::NotSupported,
            ));
        }

        Ok(SignalSet::from_bits_retain(set.bits[0]))
    }
}

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

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;

    use roxy_signal::Signal;
    use roxy_test::kernel_test;

    use super::SignalSet;

    kernel_test!("roxy-syscall::signal-set", converts_to_signal_vector, {
        let set = SignalSet::TERMINATE | SignalSet::INTERRUPT;

        assert_eq!(set.to_vec(), vec![Signal::Interrupt, Signal::Terminate]);
    });
}
