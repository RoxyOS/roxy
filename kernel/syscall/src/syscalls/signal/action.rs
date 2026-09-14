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
const SA_BASE: u32 = 1 << 8;
const SA_UNSUPPORTED: u32 = 0x80;
/// `SA_SIGINFO`: invoke the handler with `(signo, siginfo_t *, null)`.
const SA_SIGINFO: u32 = SA_BASE;
/// `SA_RESTART`: an interrupted blocking syscall is re-executed after the handler returns.
const SA_RESTART: u32 = SA_BASE << 1;

/// The Roxy `struct sigaction` record.
///
/// Layout per `sysdeps/roxy/include/abi-bits/signal.h`; sizes pinned by the assertions below.
/// POSIX declares `sa_flags` as `int`, so the flag word is four bytes wide and the alignment bytes
/// that follow it are a field of their own. Reading them as the word's high bits would hand the
/// kernel a value the caller never wrote: a caller that sets `sa_flags` has no reason to initialize
/// its record's padding, and the bytes there are whatever the stack held before.
#[repr(C)]
#[derive(Clone, Copy)]
struct SigactionAbi {
    handler: u64,
    flags: u32,
    padding: u32,
    restorer: u64,
    mask: SignalSet,
}

const _: () = assert!(mem::size_of::<SigactionAbi>() == 32);
const _: () = assert!(mem::offset_of!(SigactionAbi, handler) == 0);
const _: () = assert!(mem::offset_of!(SigactionAbi, flags) == 8);
const _: () = assert!(mem::offset_of!(SigactionAbi, padding) == 12);
const _: () = assert!(mem::offset_of!(SigactionAbi, restorer) == 16);
const _: () = assert!(mem::offset_of!(SigactionAbi, mask) == 24);

impl SyscallArg for SigactionAbi {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = roxy_memory::UserAddress::parse(raw, error)?;
        let mut value = Self {
            handler: 0,
            flags: 0,
            padding: 0,
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
    let flags = value.flags;

    if flags & SA_UNSUPPORTED != 0 {
        return Err(unsupported_argument(
            "sigaction.flags.unsupported",
            flags,
            Errno::NotSupported,
        ));
    }

    let foreign = flags & (SA_BASE - 1);
    if foreign != 0 {
        return Err(unsupported_argument(
            "sigaction.flags.foreign",
            foreign,
            Errno::NotSupported,
        ));
    }

    let unknown = flags & !(SA_SIGINFO | SA_RESTART);
    if unknown != 0 {
        return Err(unsupported_argument(
            "sigaction.flags",
            unknown,
            Errno::NotSupported,
        ));
    }

    let include_siginfo = flags & SA_SIGINFO != 0;
    let restart = flags & SA_RESTART != 0;

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
        padding: 0,
        restorer: 0,
        mask,
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_process::SignalAction;
    use roxy_signal::SignalSet;
    use roxy_test::kernel_test;

    use super::{SA_BASE, SA_RESTART, SA_SIGINFO, SA_UNSUPPORTED, SigactionAbi, decode};
    use crate::errno::Errno;

    /// The bytes a caller that sets `sa_flags` to `flags` leaves in the record.
    ///
    /// The flag word is the C record's `int sa_flags` at byte 8, and the four bytes after it belong
    /// to no field, so a caller initializes only the first four of them. The image poisons the rest,
    /// so a reader that takes them for the flag word's high bits cannot pass.
    fn caller_record(handler: u64, flags: u32) -> [u8; 32] {
        let mut record = [0u8; 32];
        record[0..8].copy_from_slice(&handler.to_le_bytes());
        record[8..12].copy_from_slice(&flags.to_le_bytes());
        record[12..16].copy_from_slice(&0x7fff_u32.to_le_bytes());
        record
    }

    /// Copies a caller's bytes into the record the way the syscall's argument read does.
    fn read_record(bytes: [u8; 32]) -> SigactionAbi {
        let mut value = core::mem::MaybeUninit::<SigactionAbi>::uninit();

        // SAFETY: `bytes` is one record long, and the copy initializes exactly that many bytes.
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                value.as_mut_ptr().cast::<u8>(),
                bytes.len(),
            );
        }

        // SAFETY: the record is integers and a set, so every byte pattern is a valid value.
        unsafe { value.assume_init() }
    }

    kernel_test!(
        "roxy-syscall::sigaction-record",
        reads_the_flag_word_without_the_records_padding,
        {
            // `signal()` zeroes the flag word and never initializes the record's padding, which is
            // why its disposition is the default one and not a request for an undefined flag.
            assert_eq!(read_record(caller_record(0, 0)).flags, 0);
            assert_eq!(
                decode(read_record(caller_record(0, 0))),
                Ok(SignalAction::Default)
            );

            // A handler installed with the two flags the kernel honours keeps both of them.
            assert_eq!(
                decode(read_record(caller_record(0x4000, SA_SIGINFO | SA_RESTART))),
                Ok(SignalAction::Handler {
                    address: 0x4000,
                    mask: SignalSet::empty(),
                    include_siginfo: true,
                    restart: true,
                })
            );
        }
    );

    kernel_test!(
        "roxy-syscall::sigaction-flag-word",
        judges_only_the_flag_word,
        {
            // Linux numbers its own flags from bit 0, so a low one is another personality's
            // numbering, and its `SA_RESTART` at bit 28 names no flag of ours either.
            assert_eq!(
                decode(read_record(caller_record(0x4000, 1))),
                Err(Errno::NotSupported)
            );
            assert_eq!(
                decode(read_record(caller_record(0x4000, 0x1000_0000))),
                Err(Errno::NotSupported)
            );

            // The header's marker for a flag it defines but this kernel cannot honour, and a flag
            // word that names no flag at all.
            assert_eq!(
                decode(read_record(caller_record(0x4000, SA_UNSUPPORTED))),
                Err(Errno::NotSupported)
            );
            assert_eq!(
                decode(read_record(caller_record(0x4000, SA_BASE << 2))),
                Err(Errno::NotSupported)
            );
        }
    );
}
