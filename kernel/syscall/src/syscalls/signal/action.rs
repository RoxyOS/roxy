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
/// Roxy numbers the `sigaction` flags from a base above Linux's own (its lowest are 1, 2, 4 and
/// its highest fill bits 24-31), so a value below the base is another personality's numbering and
/// every flag the header defines but this kernel cannot honour is its marker — which is checked as
/// a bit, so a caller that ORs it into a supported flag is still recognised. See
/// `abi-bits/signal.h`.
const SA_BASE: u64 = 1 << 8;
const SA_UNSUPPORTED: u64 = 0x80;
/// `SA_SIGINFO`: invoke the handler with `(signo, siginfo_t *, null)`.
const SA_SIGINFO: u64 = SA_BASE;
/// `SA_RESTART`: an interrupted blocking syscall is re-executed after the handler returns.
const SA_RESTART: u64 = SA_BASE << 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct SigactionAbi {
    handler: u64,
    flags: u64,
    restorer: u64,
    mask: SignalSet,
}

const _: () = assert!(mem::size_of::<SigactionAbi>() == 32);
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
            mask: SignalSet::empty(),
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
        Some(value) => Some(decode(value)?),
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

fn decode(value: SigactionAbi) -> Result<SignalAction, Errno> {
    // The Roxy ABI defines `SA_SIGINFO` and `SA_RESTART`; the header gives every other flag it
    // defines one marker, and a value below the base is another personality's numbering. All three
    // cases are reported through the centralized diagnostic.
    if value.flags & SA_UNSUPPORTED != 0 {
        return Err(unsupported_argument(
            "sigaction.flags.unsupported",
            value.flags,
            Errno::NotSupported,
        ));
    }

    let foreign = value.flags & (SA_BASE - 1);
    if foreign != 0 {
        return Err(unsupported_argument(
            "sigaction.flags.foreign",
            foreign,
            Errno::NotSupported,
        ));
    }

    let unknown = value.flags & !(SA_SIGINFO | SA_RESTART);
    if unknown != 0 {
        return Err(unsupported_argument(
            "sigaction.flags",
            unknown,
            Errno::NotSupported,
        ));
    }

    let include_siginfo = value.flags & SA_SIGINFO != 0;
    let restart = value.flags & SA_RESTART != 0;

    // The kernel injects its own sigreturn trampoline, so a user-supplied restorer is never
    // required or consulted.
    let mask = value.mask;

    Ok(match value.handler {
        0 => SignalAction::Default,
        1 => SignalAction::Ignore,
        address => SignalAction::Handler {
            address,
            mask,
            include_siginfo,
            restart,
        },
    })
}

fn encode(action: SignalAction) -> SigactionAbi {
    let (handler, mask, flags) = match action {
        SignalAction::Default => (0, SignalSet::empty(), 0),
        SignalAction::Ignore => (1, SignalSet::empty(), 0),
        SignalAction::Handler {
            address,
            mask,
            include_siginfo,
            restart,
        } => {
            let mut flags = 0;
            if include_siginfo {
                flags |= SA_SIGINFO;
            }
            if restart {
                flags |= SA_RESTART;
            }
            (address, mask, flags)
        }
    };

    SigactionAbi {
        handler,
        flags,
        restorer: 0,
        mask,
    }
}
