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
