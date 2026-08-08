use alloc::vec::Vec;
use core::mem;

use bitflags::bitflags;
use roxy_memory::UserAddress;
use roxy_signal::Signal;
use strum::IntoEnumIterator;

use crate::{
    SyscallResult,
    args::{SyscallArg, user_memory},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
    unsupported::unsupported_argument,
};

#[derive(Clone, Copy)]
enum SignalMaskHow {
    Block,
    Unblock,
    SetMask,
}

impl SignalMaskHow {
    const fn number(self) -> u64 {
        match self {
            Self::Block => 0,
            Self::Unblock => 1,
            Self::SetMask => 2,
        }
    }
}

impl SyscallArg for SignalMaskHow {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        match raw {
            0 => Ok(Self::Block),
            1 => Ok(Self::Unblock),
            2 => Ok(Self::SetMask),
            _ => Err(error),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SignalMaskAbi {
    bits: [u64; 16],
}

const _: () = assert!(mem::size_of::<SignalMaskAbi>() == 128);

const fn signal_bit(signal: Signal) -> u64 {
    1 << (signal.number() - 1)
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct SignalMask: u64 {
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

impl SignalMask {
    pub(crate) fn to_vec(self) -> Vec<Signal> {
        Signal::iter()
            .filter(|signal| self.bits() & signal_bit(*signal) != 0)
            .collect()
    }
}

impl SyscallArg for SignalMask {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut mask = SignalMaskAbi { bits: [0; 16] };

        // SAFETY: SignalMaskAbi has a checked C layout and is fully initialized.
        unsafe { user_memory::read(address, &mut mask) }?;

        if mask.bits[1..].iter().any(|bits| *bits != 0) {
            return Err(unsupported_argument(
                "signal_mask.extended_bits",
                "set",
                Errno::NotSupported,
            ));
        }

        Ok(SignalMask::from_bits_retain(mask.bits[0]))
    }
}

syscall!(SyscallNumber::Sigprocmask, handle(how: SignalMaskHow => Invalid, mask: SignalMask => Fault));

fn handle(how: SignalMaskHow, _mask: SignalMask) -> SyscallResult {
    Err(unsupported_argument(
        "sigprocmask",
        how.number(),
        Errno::NoSys,
    ))
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;

    use roxy_signal::Signal;
    use roxy_test::kernel_test;

    use super::SignalMask;

    kernel_test!("roxy-syscall::signal-mask", converts_to_signal_vector, {
        let mask = SignalMask::TERMINATE | SignalMask::INTERRUPT;

        assert_eq!(mask.to_vec(), vec![Signal::Interrupt, Signal::Terminate]);
    });
}
