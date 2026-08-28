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
    Trap = 5,
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
    StackFault = 16,
    Urgent = 23,
    CpuLimit = 24,
    FileSizeLimit = 25,
    VirtualAlarm = 26,
    ProfilingAlarm = 27,
    WindowChanged = 28,
    Io = 29,
    PowerFailure = 30,
    SystemCall = 31,
    /// POSIX reserved cancellation signal (Linux `SIGCANCEL`, the first reserved real-time
    /// number). Disposed of by the pthread runtime when it installs its cancellation handler.
    Cancellation = 32,
    /// Linux `SIGTIMER`, the second reserved real-time number.
    Timer = 33,
    RealTime1 = 34,
    RealTime2 = 35,
    RealTime3 = 36,
    RealTime4 = 37,
    RealTime5 = 38,
    RealTime6 = 39,
    RealTime7 = 40,
    RealTime8 = 41,
    RealTime9 = 42,
    RealTime10 = 43,
    RealTime11 = 44,
    RealTime12 = 45,
    RealTime13 = 46,
    RealTime14 = 47,
    RealTime15 = 48,
    RealTime16 = 49,
    RealTime17 = 50,
    RealTime18 = 51,
    RealTime19 = 52,
    RealTime20 = 53,
    RealTime21 = 54,
    RealTime22 = 55,
    RealTime23 = 56,
    RealTime24 = 57,
    RealTime25 = 58,
    RealTime26 = 59,
    RealTime27 = 60,
    RealTime28 = 61,
    RealTime29 = 62,
    RealTime30 = 63,
    RealTime31 = 64,
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
            | Self::Terminate
            | Self::Trap
            | Self::StackFault
            | Self::CpuLimit
            | Self::FileSizeLimit
            | Self::VirtualAlarm
            | Self::ProfilingAlarm
            | Self::Io
            | Self::PowerFailure
            | Self::SystemCall
            // POSIX mandates that the default action for a real-time signal is to terminate.
            | Self::Cancellation
            | Self::Timer
            | Self::RealTime1
            | Self::RealTime2
            | Self::RealTime3
            | Self::RealTime4
            | Self::RealTime5
            | Self::RealTime6
            | Self::RealTime7
            | Self::RealTime8
            | Self::RealTime9
            | Self::RealTime10
            | Self::RealTime11
            | Self::RealTime12
            | Self::RealTime13
            | Self::RealTime14
            | Self::RealTime15
            | Self::RealTime16
            | Self::RealTime17
            | Self::RealTime18
            | Self::RealTime19
            | Self::RealTime20
            | Self::RealTime21
            | Self::RealTime22
            | Self::RealTime23
            | Self::RealTime24
            | Self::RealTime25
            | Self::RealTime26
            | Self::RealTime27
            | Self::RealTime28
            | Self::RealTime29
            | Self::RealTime30
            | Self::RealTime31 => DefaultAction::Terminate,
            Self::Child | Self::WindowChanged | Self::Urgent => DefaultAction::Ignore,
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
                Signal::Trap,
                Signal::StackFault,
                Signal::CpuLimit,
                Signal::FileSizeLimit,
                Signal::VirtualAlarm,
                Signal::ProfilingAlarm,
                Signal::Io,
                Signal::PowerFailure,
                Signal::SystemCall,
            ],
            DefaultAction::Terminate,
        );
        assert_actions(
            &[Signal::Child, Signal::WindowChanged, Signal::Urgent],
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

    kernel_test!(
        "roxy-signal::realtime-default-actions",
        realtime_default_actions,
        {
            assert_actions(
                &[
                    Signal::Cancellation,
                    Signal::Timer,
                    Signal::RealTime1,
                    Signal::RealTime7,
                    Signal::RealTime16,
                    Signal::RealTime31,
                ],
                DefaultAction::Terminate,
            );
            assert_eq!(Signal::Cancellation.number(), 32);
            assert_eq!(Signal::RealTime1.number(), 34);
            assert_eq!(Signal::RealTime31.number(), 64);
        }
    );
}
