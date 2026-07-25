use alloc::vec::Vec;
use core::ops::Deref;

use roxy_memory::{PAGE_SIZE, UserAddress};
use roxy_vfs::ResolvedPath;

use super::SyscallArg;
use crate::errno::Errno;

const MAX_LENGTH: usize = ResolvedPath::MAX_LEN;

pub(crate) struct CString(Vec<u8>);

impl CString {
    pub(crate) fn from_address(address: UserAddress) -> Result<Self, Errno> {
        let addrspace = roxy_process::current_addrspace().map_err(|_| Errno::Fault)?;
        let mut bytes = Vec::new();

        while bytes.len() < MAX_LENGTH {
            let current = address
                .checked_add(u64::try_from(bytes.len()).map_err(|_| Errno::Fault)?)
                .ok_or(Errno::Fault)?;
            let page_remaining = usize::try_from(PAGE_SIZE - current.as_u64() % PAGE_SIZE).unwrap();
            let length = page_remaining.min(MAX_LENGTH - bytes.len());
            let start = bytes.len();

            bytes.resize(start + length, 0);
            addrspace
                .read_bytes(current, &mut bytes[start..])
                .map_err(|_| Errno::Fault)?;

            if let Some(terminator) = bytes[start..].iter().position(|byte| *byte == 0) {
                bytes.truncate(start + terminator);

                return Ok(Self(bytes));
            }
        }

        Err(Errno::NameTooLong)
    }

    pub(crate) fn into_inner(self) -> Vec<u8> {
        self.0
    }
}

impl Deref for CString {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SyscallArg for CString {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let address = UserAddress::parse(raw, error)?;

        Self::from_address(address)
    }
}

#[cfg(feature = "kernel-test")]
mod tests {
    use alloc::vec;
    use roxy_test::kernel_test;

    use super::CString;

    kernel_test!("roxy-syscall::c-string-deref", dereferences_to_inner, {
        let string = CString(vec![b'a', b'b']);

        assert_eq!(&**string, b"ab");
    });
}
