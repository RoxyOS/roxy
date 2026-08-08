use crate::{
    SyscallResult, args::SyscallArg, errno::Errno, numbers::SyscallNumber, syscall,
    unsupported::unsupported_argument,
};

use super::SignalSet;

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

syscall!(SyscallNumber::Sigprocmask, handle(how: SignalMaskHow => Invalid, set: SignalSet => Fault));

fn handle(how: SignalMaskHow, _set: SignalSet) -> SyscallResult {
    Err(unsupported_argument(
        "sigprocmask",
        how.number(),
        Errno::NoSys,
    ))
}
