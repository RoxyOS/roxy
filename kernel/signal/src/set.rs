use alloc::vec::Vec;
use bitflags::bitflags;
use strum::IntoEnumIterator;

use super::Signal;

const fn bit(signal: Signal) -> u64 {
    1 << (signal.number() - 1)
}

bitflags! {
    /// A set of signals, one bit per supported signal.
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub struct SignalSet: u64 {
        const HANGUP = bit(Signal::Hangup);
        const INTERRUPT = bit(Signal::Interrupt);
        const QUIT = bit(Signal::Quit);
        const ILLEGAL_INSTRUCTION = bit(Signal::IllegalInstruction);
        const ABORT = bit(Signal::Abort);
        const BUS_ERROR = bit(Signal::BusError);
        const FLOATING_POINT_EXCEPTION = bit(Signal::FloatingPointException);
        const KILL = bit(Signal::Kill);
        const USER1 = bit(Signal::User1);
        const SEGMENTATION_FAULT = bit(Signal::SegmentationFault);
        const USER2 = bit(Signal::User2);
        const BROKEN_PIPE = bit(Signal::BrokenPipe);
        const ALARM = bit(Signal::Alarm);
        const TERMINATE = bit(Signal::Terminate);
        const CHILD = bit(Signal::Child);
        const CONTINUE = bit(Signal::Continue);
        const STOP = bit(Signal::Stop);
        const TERMINAL_STOP = bit(Signal::TerminalStop);
        const TERMINAL_INPUT = bit(Signal::TerminalInput);
        const TERMINAL_OUTPUT = bit(Signal::TerminalOutput);
        const WINDOW_CHANGED = bit(Signal::WindowChanged);
    }
}

impl SignalSet {
    /// Converts this set into the signals it contains.
    #[must_use]
    pub fn to_vec(self) -> Vec<Signal> {
        Signal::iter()
            .filter(|signal| self.contains(Self::from_signal(*signal)))
            .collect()
    }

    /// Builds a set from the given signals.
    #[must_use]
    pub fn from_signals(signals: &[Signal]) -> Self {
        let mut set = Self::empty();

        for signal in signals {
            set.insert(Self::from_signal(*signal));
        }

        set
    }

    /// Builds a single-signal set.
    #[must_use]
    pub const fn from_signal(signal: Signal) -> Self {
        Self::from_bits_retain(bit(signal))
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;

    use roxy_test::kernel_test;

    use super::{Signal, SignalSet};

    kernel_test!(
        "roxy-signal::signal-set",
        converts_between_signals_and_set,
        {
            let set = SignalSet::TERMINATE | SignalSet::INTERRUPT;

            assert_eq!(set.to_vec(), vec![Signal::Interrupt, Signal::Terminate]);
            assert_eq!(SignalSet::from_signals(&set.to_vec()), set);
        }
    );

    kernel_test!("roxy-signal::signal-set", builds_single_signal_sets, {
        assert_eq!(SignalSet::from_signal(Signal::Kill), SignalSet::KILL);
        assert!(SignalSet::empty().to_vec().is_empty());
    });
}
