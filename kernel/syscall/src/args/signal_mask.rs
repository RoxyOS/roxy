use alloc::vec::Vec;
use core::mem;

use bitflags::bitflags;
use roxy_memory::UserAddress;
use roxy_signal::Signal;
use strum::IntoEnumIterator;

use super::{SyscallArg, user_memory};
use crate::{errno::Errno, unsupported::unsupported_argument};

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
        let mut signals = Vec::new();

        for signal in Signal::iter() {
            if self.bits() & signal_bit(signal) != 0 {
                signals.push(signal);
            }
        }

        signals
    }
}

impl SyscallArg for SignalMask {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut mask = SignalMaskAbi { bits: [0; 16] };

        // SAFETY: SignalMaskAbi has a checked C layout, its integer fields accept every bit
        // pattern, and the output is fully initialized before userspace copies into it.
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

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;

    use roxy_signal::Signal;
    use roxy_test::kernel_test;

    use super::SignalMask;

    kernel_test!(
        "roxy-syscall::signal-mask-vector",
        converts_to_signal_vector,
        {
            let mask = SignalMask::TERMINATE | SignalMask::INTERRUPT;

            assert_eq!(mask.to_vec(), vec![Signal::Interrupt, Signal::Terminate]);
        }
    );
}
