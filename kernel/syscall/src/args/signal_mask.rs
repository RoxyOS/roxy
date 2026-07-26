use core::mem;

use roxy_memory::UserAddress;

use super::{SyscallArg, user_memory};
use crate::errno::Errno;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct SignalMask {
    bits: [u64; 16],
}

const _: () = assert!(mem::size_of::<SignalMask>() == 128);

impl SignalMask {
    #[must_use]
    pub(crate) fn is_empty(self) -> bool {
        self.bits.iter().all(|bits| *bits == 0)
    }
}

impl SyscallArg for SignalMask {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;
        let mut mask = Self { bits: [0; 16] };

        // SAFETY: SignalMask has a checked C layout, its integer fields accept every bit
        // pattern, and the output is fully initialized before userspace copies into it.
        unsafe { user_memory::read(address, &mut mask) }?;

        Ok(mask)
    }
}
