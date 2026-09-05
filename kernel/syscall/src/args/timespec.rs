use core::{mem, time::Duration};

use roxy_memory::UserAddress;

use super::{SyscallArg, user_memory};
use crate::errno::Errno;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Timespec {
    seconds: i64,
    nanoseconds: i64,
}

const _: () = assert!(mem::size_of::<Timespec>() == 16);

impl Timespec {
    /// Builds a timespec value from explicit parts.
    #[must_use]
    pub(crate) const fn new(seconds: i64, nanoseconds: i64) -> Self {
        Self {
            seconds,
            nanoseconds,
        }
    }

    /// Nanoseconds are validated to `[0, 1_000_000_000)` during parsing, so the truncating cast
    /// cannot lose information.
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub(crate) const fn duration(self) -> Duration {
        Duration::new(
            self.seconds.cast_unsigned(),
            self.nanoseconds.cast_unsigned() as u32,
        )
    }

    /// Returns whether this is a valid nonnegative `timespec`: nonnegative seconds with
    /// nanoseconds in `[0, 1_000_000_000)`.
    #[must_use]
    pub(crate) fn is_valid(self) -> bool {
        self.seconds >= 0 && (0..1_000_000_000).contains(&self.nanoseconds)
    }

    /// Builds a timespec from a nonnegative [`Duration`].
    #[must_use]
    pub(crate) fn from_duration(duration: Duration) -> Self {
        Self {
            seconds: i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
            nanoseconds: i64::from(duration.subsec_nanos()),
        }
    }
}

impl SyscallArg for Timespec {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut timespec = Self {
            seconds: 0,
            nanoseconds: 0,
        };

        // SAFETY: TimespecAbi has a checked C layout, contains only integers, and accepts every
        // bit pattern copied from userspace.
        unsafe { user_memory::read(address, &mut timespec) }?;

        if timespec.seconds < 0 || !(0..1_000_000_000).contains(&timespec.nanoseconds) {
            return Err(Errno::Invalid);
        }

        Ok(timespec)
    }
}
