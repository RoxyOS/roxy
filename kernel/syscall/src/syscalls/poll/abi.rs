//! The `poll` request record and the two condition words it carries.
//!
//! Layout per `sysdeps/roxy/include/roxy/syscall.h`; sizes pinned by the assertions below.

use core::mem::{align_of, offset_of, size_of};

use bitflags::bitflags;
use roxy_fd::PollEvents;

use crate::errno::Errno;

/// One descriptor a `poll` request asks about.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct PollRequestAbi {
    /// The descriptor to ask about, or a negative value to report nothing for this entry, which
    /// POSIX `poll` allows.
    pub(super) fd: i32,
    /// The conditions the caller waits for, one [`PollRequestedEvents`] bit each. A bit outside
    /// that word names no condition this kernel serves, which
    /// [`PollRequestedEvents::from_record`] reports.
    pub(super) requested_events: u32,
    /// The conditions the kernel observed to hold, one [`PollReportedEvents`] bit each. The kernel
    /// replaces whatever the caller sent here.
    pub(super) reported_events: u32,
}

const _: () = assert!(size_of::<PollRequestAbi>() == 12);
const _: () = assert!(align_of::<PollRequestAbi>() == 4);
const _: () = assert!(offset_of!(PollRequestAbi, fd) == 0);
const _: () = assert!(offset_of!(PollRequestAbi, requested_events) == 4);
const _: () = assert!(offset_of!(PollRequestAbi, reported_events) == 8);

bitflags! {
    /// The conditions a caller can wait for.
    ///
    /// These are exactly the conditions a request may name: `ERROR`, `HANGUP`, and
    /// `INVALID_DESCRIPTOR` are reported whether or not they were asked for, so no request can ask
    /// for them and they belong to [`PollReportedEvents`] alone.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct PollRequestedEvents: u32 {
        const READABLE = 1 << 0;
        const PRIORITY = 1 << 1;
        const WRITABLE = 1 << 2;
    }
}

bitflags! {
    /// The conditions a `poll` entry reports as holding.
    ///
    /// The first three are the same bits as in [`PollRequestedEvents`], so a reported condition and
    /// the request for it are one bit and the reported set is a subset of the requested one.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) struct PollReportedEvents: u32 {
        const READABLE = 1 << 0;
        const PRIORITY = 1 << 1;
        const WRITABLE = 1 << 2;
        const ERROR = 1 << 3;
        const HANGUP = 1 << 4;
        const INVALID_DESCRIPTOR = 1 << 5;
    }
}

impl PollRequestedEvents {
    /// Decodes one entry's request word, reporting a bit that names no condition this kernel
    /// serves.
    ///
    /// The word is a field of Roxy's own record rather than a word a caller shares with another
    /// personality, so no value below a base can arrive and there is no foreign numbering to
    /// separate from ours: every bit outside this type is undefined, and is reported as such.
    pub(super) fn from_record(word: u32) -> Result<Self, Errno> {
        Self::from_bits(word).ok_or_else(|| {
            crate::unsupported::unsupported_argument(
                "poll.requested_events",
                u64::from(word),
                Errno::Invalid,
            )
        })
    }
}

/// Reports the conditions that hold for one descriptor.
///
/// A requested condition is reported only when it holds, while `ERROR` and `HANGUP` are reported
/// whatever the request named, which is what makes a caller observe a failure or a closed peer it
/// did not ask about.
pub(super) fn reported_events(
    requested: PollRequestedEvents,
    events: PollEvents,
) -> PollReportedEvents {
    let mut reported = PollReportedEvents::empty();

    if requested.contains(PollRequestedEvents::READABLE) && events.readable {
        reported.insert(PollReportedEvents::READABLE);
    }

    if requested.contains(PollRequestedEvents::PRIORITY) && events.priority {
        reported.insert(PollReportedEvents::PRIORITY);
    }

    if requested.contains(PollRequestedEvents::WRITABLE) && events.writable {
        reported.insert(PollReportedEvents::WRITABLE);
    }

    if events.error {
        reported.insert(PollReportedEvents::ERROR);
    }

    if events.hangup {
        reported.insert(PollReportedEvents::HANGUP);
    }

    reported
}

#[cfg(feature = "kernel-test")]
mod tests {
    use roxy_fd::PollEvents;
    use roxy_test::kernel_test;

    use super::{PollReportedEvents, PollRequestedEvents, reported_events};

    kernel_test!(
        "roxy-syscall::poll-condition-codec",
        encodes_held_requested_conditions,
        {
            let reported = reported_events(
                PollRequestedEvents::READABLE | PollRequestedEvents::WRITABLE,
                PollEvents {
                    readable: true,
                    writable: true,
                    hangup: true,
                    ..PollEvents::default()
                },
            );

            assert_eq!(
                reported,
                PollReportedEvents::READABLE
                    | PollReportedEvents::WRITABLE
                    | PollReportedEvents::HANGUP
            );
        }
    );

    kernel_test!(
        "roxy-syscall::poll-condition-filter",
        withholds_unrequested_conditions,
        {
            let reported = reported_events(
                PollRequestedEvents::WRITABLE,
                PollEvents {
                    readable: true,
                    ..PollEvents::default()
                },
            );

            assert_eq!(reported, PollReportedEvents::empty());
        }
    );

    kernel_test!(
        "roxy-syscall::poll-request-word",
        rejects_undefined_request_bits,
        {
            assert_eq!(
                PollRequestedEvents::from_record(PollRequestedEvents::READABLE.bits()),
                Ok(PollRequestedEvents::READABLE)
            );
            assert!(PollRequestedEvents::from_record(1 << 9).is_err());
            assert!(PollRequestedEvents::from_record(PollReportedEvents::ERROR.bits()).is_err());
        }
    );
}
