use core::mem;

use roxy_process::SignalAction;
use roxy_signal::{Signal, SignalSet};

use crate::{
    SyscallResult,
    args::{Nullable, Out, SyscallArg, user_memory},
    errno::Errno,
    numbers::SyscallNumber,
    syscall,
    unsupported::unsupported_argument,
};

use super::SignalSetAbi;

#[repr(C)]
#[derive(Clone, Copy)]
struct SigactionAbi {
    handler: u64,
    flags: u64,
    restorer: u64,
    mask: SignalSetAbi,
}

const _: () = assert!(mem::size_of::<SigactionAbi>() == 152);
const _: () = assert!(mem::offset_of!(SigactionAbi, handler) == 0);
const _: () = assert!(mem::offset_of!(SigactionAbi, flags) == 8);
const _: () = assert!(mem::offset_of!(SigactionAbi, restorer) == 16);
const _: () = assert!(mem::offset_of!(SigactionAbi, mask) == 24);

impl SyscallArg for SigactionAbi {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = roxy_memory::UserAddress::parse(raw, error)?;
        let mut value = Self {
            handler: 0,
            flags: 0,
            restorer: 0,
            mask: SignalSetAbi { bits: [0; 16] },
        };

        // SAFETY: SigactionAbi has a checked C layout and is fully initialized before the copy.
        unsafe { user_memory::read(address, &mut value) }?;

        Ok(value)
    }
}

syscall!(
    SyscallNumber::Sigaction,
    handle(
        signal: Signal => Invalid,
        newact: Nullable<SigactionAbi> => Fault,
        oldact: Nullable<Out<SigactionAbi>> => Fault
    )
);

fn handle(
    signal: Signal,
    newact: Nullable<SigactionAbi>,
    oldact: Nullable<Out<SigactionAbi>>,
) -> SyscallResult {
    let oldact = oldact.into_option();
    if let Some(output) = oldact {
        output.validate()?;
    }

    let new_action = match newact.into_option() {
        Some(value) => Some(decode(value, signal)?),
        None => None,
    };

    let old_action = roxy_process::signal_action_of(signal);

    if let Some(new_action) = new_action {
        roxy_process::replace_signal_action(signal, new_action).map_err(|_| {
            unsupported_argument("sigaction.action", signal.number(), Errno::NotSupported)
        })?;
    }

    if let Some(output) = oldact {
        let value = encode(old_action);
        // SAFETY: SigactionAbi has a checked C layout and every field is initialized.
        unsafe { output.write(&value) }?;
    }

    Ok(0)
}

fn decode(value: SigactionAbi, signal: Signal) -> Result<SignalAction, Errno> {
    // Only `SA_SIGINFO` is defined in the Roxy ABI today, and handler delivery with a siginfo
    // frame is not implemented; everything else is rejected through the centralized diagnostic.
    if value.flags != 0 {
        return Err(unsupported_argument(
            "sigaction.flags",
            signal.number(),
            Errno::NotSupported,
        ));
    }

    // The kernel injects its own sigreturn trampoline, so a user-supplied restorer is never
    // required or consulted.
    let mask = value.mask.to_set(signal)?;

    Ok(match value.handler {
        0 => SignalAction::Default,
        1 => SignalAction::Ignore,
        address => SignalAction::Handler { address, mask },
    })
}

fn encode(action: SignalAction) -> SigactionAbi {
    let (handler, mask) = match action {
        SignalAction::Default => (0, SignalSet::empty()),
        SignalAction::Ignore => (1, SignalSet::empty()),
        SignalAction::Handler { address, mask } => (address, mask),
    };

    SigactionAbi {
        handler,
        flags: 0,
        restorer: 0,
        mask: SignalSetAbi::from_set(mask),
    }
}
