use alloc::vec::Vec;

use super::{CString, SyscallArg};
use crate::errno::Errno;

pub(crate) struct Path(CString);

impl Path {
    pub(crate) fn into_inner(self) -> Vec<u8> {
        self.0.into_inner()
    }
}

impl SyscallArg for Path {
    fn parse(raw: u64, error: Errno) -> Result<Self, Errno> {
        let path = CString::parse(raw, error)?;

        if path.is_empty() {
            return Err(Errno::NotFound);
        }

        Ok(Self(path))
    }
}
