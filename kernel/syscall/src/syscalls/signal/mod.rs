mod action;
mod mask;
mod send;
mod sigreturn;

use core::mem;

use crate::{
    Syscall,
    args::{SyscallArg, user_memory},
    errno::Errno,
    unsupported::unsupported_argument,
};
use roxy_memory::UserAddress;
use roxy_signal::{Signal, SignalSet};

pub(super) const ACTION_SYSCALL: Syscall = action::SYSCALL;
pub(super) const MASK_SYSCALL: Syscall = mask::SYSCALL;
pub(super) const SEND_SYSCALL: Syscall = send::SYSCALL;
pub(super) const SIGRETURN_SYSCALL: Syscall = sigreturn::SYSCALL;

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct SignalSetAbi {
    bits: [u64; 16],
}

const _: () = assert!(mem::size_of::<SignalSetAbi>() == 128);

impl SignalSetAbi {
    pub(super) const fn from_set(set: SignalSet) -> Self {
        let mut bits = [0; 16];
        bits[0] = set.bits();
        Self { bits }
    }

    pub(super) fn to_set(self, signal: Signal) -> Result<SignalSet, Errno> {
        if self.bits[1..].iter().any(|bits| *bits != 0) {
            return Err(unsupported_argument(
                "signal_set.extended_bits",
                signal.number(),
                Errno::NotSupported,
            ));
        }

        Ok(SignalSet::from_bits_retain(self.bits[0]))
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
            5 => Signal::Trap,
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
            16 => Signal::StackFault,
            17 => Signal::Child,
            18 => Signal::Continue,
            19 => Signal::Stop,
            20 => Signal::TerminalStop,
            21 => Signal::TerminalInput,
            22 => Signal::TerminalOutput,
            23 => Signal::Urgent,
            24 => Signal::CpuLimit,
            25 => Signal::FileSizeLimit,
            26 => Signal::VirtualAlarm,
            27 => Signal::ProfilingAlarm,
            28 => Signal::WindowChanged,
            29 => Signal::Io,
            30 => Signal::PowerFailure,
            31 => Signal::SystemCall,
            32 => Signal::Cancellation,
            33 => Signal::Timer,
            34 => Signal::RealTime1,
            35 => Signal::RealTime2,
            36 => Signal::RealTime3,
            37 => Signal::RealTime4,
            38 => Signal::RealTime5,
            39 => Signal::RealTime6,
            40 => Signal::RealTime7,
            41 => Signal::RealTime8,
            42 => Signal::RealTime9,
            43 => Signal::RealTime10,
            44 => Signal::RealTime11,
            45 => Signal::RealTime12,
            46 => Signal::RealTime13,
            47 => Signal::RealTime14,
            48 => Signal::RealTime15,
            49 => Signal::RealTime16,
            50 => Signal::RealTime17,
            51 => Signal::RealTime18,
            52 => Signal::RealTime19,
            53 => Signal::RealTime20,
            54 => Signal::RealTime21,
            55 => Signal::RealTime22,
            56 => Signal::RealTime23,
            57 => Signal::RealTime24,
            58 => Signal::RealTime25,
            59 => Signal::RealTime26,
            60 => Signal::RealTime27,
            61 => Signal::RealTime28,
            62 => Signal::RealTime29,
            63 => Signal::RealTime30,
            64 => Signal::RealTime31,
            0 => return Err(unsupported_argument("signal", raw, Errno::NotSupported)),
            _ => return Err(error),
        };

        Ok(signal)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_signal::SignalSet;
    use roxy_test::kernel_test;

    use super::SignalSetAbi;
    use crate::numbers::SyscallNumber;

    kernel_test!("roxy-syscall::signal-set", round_trips_through_abi, {
        let set = SignalSet::TERMINATE | SignalSet::INTERRUPT;

        assert_eq!(SignalSetAbi::from_set(set).bits[0], set.bits());
    });

    kernel_test!(
        "roxy-syscall::sigreturn-number",
        matches_process_trampoline,
        {
            assert_eq!(
                roxy_process::SIGRETURN_SYSCALL_NUMBER,
                SyscallNumber::Sigreturn as u64
            );
        }
    );

    kernel_test!("roxy-syscall::signal-number", parses_realtime_signals, {
        use crate::{args::SyscallArg, errno::Errno};
        use roxy_signal::Signal;

        assert_eq!(
            <Signal as SyscallArg>::parse(32, Errno::Invalid),
            Ok(Signal::Cancellation)
        );
        assert_eq!(
            <Signal as SyscallArg>::parse(64, Errno::Invalid),
            Ok(Signal::RealTime31)
        );
        assert!(<Signal as SyscallArg>::parse(0, Errno::Invalid).is_err());
    });
}
