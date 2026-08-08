use alloc::vec::Vec;

use crate::{
    SyscallResult,
    args::{Nullable, Out, SyscallArg},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
};

use super::{SignalSet, SignalSetAbi};

#[derive(Clone, Copy)]
enum SignalMaskHow {
    Block,
    Unblock,
    SetMask,
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

syscall!(SyscallNumber::Sigprocmask, handle(how: SignalMaskHow => Invalid, set: Nullable<SignalSet> => Fault, old_set: Nullable<Out<SignalSetAbi>> => Fault));

fn handle(
    how: SignalMaskHow,
    set: Nullable<SignalSet>,
    old_set: Nullable<Out<SignalSetAbi>>,
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
        let old_set_value = SignalSet::from_signals(&old_signals).to_abi();
        // SAFETY: SignalSetAbi has a checked C layout and every byte is initialized.
        unsafe { old_set.write(&old_set_value) }?;
    }

    Ok(0)
}

fn update_mask(how: SignalMaskHow, set: SignalSet) -> Vec<roxy_signal::Signal> {
    let signals = set.to_vec();

    match how {
        SignalMaskHow::Block => roxy_process::block_signals(signals),
        SignalMaskHow::Unblock => roxy_process::unblock_signals(signals),
        SignalMaskHow::SetMask => roxy_process::replace_masked_signals(signals),
    }
}
