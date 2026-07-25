use alloc::vec::Vec;
use core::mem::size_of;

use roxy_memory::UserAddress;

use super::{CString, SyscallArg, user_memory};
use crate::errno::Errno;

const MAX_ITEMS: usize = 4096;

pub(crate) struct CStringArray(Vec<Vec<u8>>);

impl CStringArray {
    pub(crate) fn from_raw(raw: u64, error: Errno) -> Result<Self, Errno> {
        if raw == 0 {
            return Ok(Self(Vec::new()));
        }

        let base = UserAddress::new(raw).ok_or(error)?;
        let mut strings = Vec::new();

        for index in 0..MAX_ITEMS {
            let offset = index.checked_mul(size_of::<u64>()).unwrap();
            let address = base
                .checked_add(u64::try_from(offset).unwrap())
                .ok_or(error)?;
            let mut pointer = 0u64;

            // SAFETY: u64 has no padding, every bit pattern is valid, and pointer is initialized.
            unsafe { user_memory::read(address, &mut pointer) }?;

            if pointer == 0 {
                return Ok(Self(strings));
            }

            let address = UserAddress::new(pointer).ok_or(error)?;
            let string = CString::from_address(address)?;

            strings.try_reserve(1).map_err(|_| Errno::NoMem)?;
            strings.push(string.into_inner());
        }

        Err(Errno::TooBig)
    }

    pub(crate) fn into_inner(self) -> Vec<Vec<u8>> {
        self.0
    }
}

impl SyscallArg for CStringArray {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        Self::from_raw(raw, error)
    }
}
