use crate::{
    SyscallResult,
    args::{Nullable, Out, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

use roxy_signal::SignalSet;

/// Roxy numbers mask operations from a base above Linux's range, so a Linux-valued `how` is
/// reported as a foreign numbering instead of being silently honoured.
const MASK_HOW_BASE: u64 = 1 << 8;

const MASK_BLOCK: u64 = MASK_HOW_BASE;
const MASK_UNBLOCK: u64 = MASK_HOW_BASE + 1;
const MASK_SETMASK: u64 = MASK_HOW_BASE + 2;

#[derive(Clone, Copy)]
enum SignalMaskHow {
    Block,
    Unblock,
    SetMask,
}

impl SyscallArg for SignalMaskHow {
    fn parse(raw: u64, _error: Errno) -> Result<Self, Errno> {
        match raw {
            MASK_BLOCK => Ok(Self::Block),
            MASK_UNBLOCK => Ok(Self::Unblock),
            MASK_SETMASK => Ok(Self::SetMask),
            value if value < MASK_HOW_BASE => Err(crate::unsupported::unsupported_argument(
                "sigprocmask.how.foreign",
                value,
                Errno::Invalid,
            )),
            value => Err(crate::unsupported::unsupported_argument(
                "sigprocmask.how",
                value,
                Errno::Invalid,
            )),
        }
    }
}

syscall!(SyscallNumber::Sigprocmask, handle(how: SignalMaskHow => Invalid, set: Nullable<SignalSet> => Fault, old_set: Nullable<Out<SignalSet>> => Fault));

fn handle(
    how: SignalMaskHow,
    set: Nullable<SignalSet>,
    old_set: Nullable<Out<SignalSet>>,
) -> SyscallResult {
    let set = set.into_option();
    let old_set = old_set.into_option();

    if let Some(old_set) = old_set {
        old_set.validate()?;
    }

    let old_signals = match set {
        None => roxy_process::currently_blocked_signals(),
        Some(set) => update_mask(how, set),
    };

    if let Some(old_set) = old_set {
        // SAFETY: `SignalSet` is one word with every byte initialized.
        unsafe { old_set.write(&old_signals) }?;
    }

    Ok(0)
}

fn update_mask(how: SignalMaskHow, set: SignalSet) -> SignalSet {
    match how {
        SignalMaskHow::Block => roxy_process::block_signals(set),
        SignalMaskHow::Unblock => roxy_process::unblock_signals(set),
        SignalMaskHow::SetMask => roxy_process::replace_masked_signals(set),
    }
}
