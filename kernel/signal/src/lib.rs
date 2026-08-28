#![no_std]
extern crate alloc;

mod action;
mod set;

pub use action::DefaultAction;
pub use set::SignalSet;
use strum::EnumIter;

/// A process-directed signal supported by the current kernel.
#[repr(u8)]
#[derive(Clone, Copy, Debug, EnumIter, Eq, Hash, PartialEq)]
pub enum Signal {
    Hangup = 1,
    Interrupt = 2,
    Quit = 3,
    IllegalInstruction = 4,
    BusError = 7,
    Abort = 6,
    FloatingPointException = 8,
    Kill = 9,
    User1 = 10,
    SegmentationFault = 11,
    User2 = 12,
    BrokenPipe = 13,
    Alarm = 14,
    Terminate = 15,
    Child = 17,
    Continue = 18,
    Stop = 19,
    TerminalStop = 20,
    TerminalInput = 21,
    TerminalOutput = 22,
    WindowChanged = 28,
}

impl Signal {
    #[must_use]
    pub const fn default_action(self) -> DefaultAction {
        match self {
            Self::Hangup
            | Self::Interrupt
            | Self::Kill
            | Self::User1
            | Self::User2
            | Self::BrokenPipe
            | Self::Alarm
            | Self::Terminate => DefaultAction::Terminate,
            Self::Child | Self::WindowChanged => DefaultAction::Ignore,
            Self::Quit
            | Self::IllegalInstruction
            | Self::BusError
            | Self::Abort
            | Self::FloatingPointException
            | Self::SegmentationFault
            | Self::Continue
            | Self::Stop
            | Self::TerminalStop
            | Self::TerminalInput
            | Self::TerminalOutput => DefaultAction::Unsupported,
        }
    }

    #[must_use]
    pub const fn number(self) -> u8 {
        self as u8
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_test::kernel_test;

    use super::{DefaultAction, Signal};

    kernel_test!("roxy-signal::default-actions", default_actions, {
        assert_actions(
            &[
                Signal::Hangup,
                Signal::Interrupt,
                Signal::Kill,
                Signal::User1,
                Signal::User2,
                Signal::BrokenPipe,
                Signal::Alarm,
                Signal::Terminate,
            ],
            DefaultAction::Terminate,
        );
        assert_actions(
            &[Signal::Child, Signal::WindowChanged],
            DefaultAction::Ignore,
        );
        assert_actions(
            &[
                Signal::Quit,
                Signal::IllegalInstruction,
                Signal::BusError,
                Signal::Abort,
                Signal::FloatingPointException,
                Signal::SegmentationFault,
                Signal::Continue,
                Signal::Stop,
                Signal::TerminalStop,
                Signal::TerminalInput,
                Signal::TerminalOutput,
            ],
            DefaultAction::Unsupported,
        );
    });

    fn assert_actions(signals: &[Signal], action: DefaultAction) {
        for signal in signals {
            assert_eq!(signal.default_action(), action);
        }
    }
}
